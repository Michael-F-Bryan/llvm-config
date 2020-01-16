//! A thin wrapper around the `llvm-config` tool so you can call it from Rust.
//!
//! This is mainly intended as a tool for build scripts which need to use LLVM
//! but don't want to manually parse the output and handle errors every time.

#![forbid(unsafe_code)]
#![deny(missing_docs, missing_debug_implementations)]

use std::{
    ffi::OsStr,
    fmt::{self, Display, Formatter},
    io,
    path::PathBuf,
    process::{Command, Output, Stdio},
    string::FromUtf8Error,
};

/// Print LLVM version.
pub fn version() -> Result<String, Error> {
    map_stdout(&["--verson"], ToString::to_string)
}

/// Print the installation prefix.
pub fn prefix() -> Result<PathBuf, Error> {
    map_stdout(&["--prefix"], |s| PathBuf::from(s))
}

/// Print the source root LLVM was built from.
pub fn src_root() -> Result<PathBuf, Error> {
    map_stdout(&["--src-root"], |s| PathBuf::from(s))
}
/// Print the object root used to build LLVM.
pub fn obj_root() -> Result<PathBuf, Error> {
    map_stdout(&["--obj-root"], |s| PathBuf::from(s))
}

/// Directory containing LLVM executables.
pub fn bin_dir() -> Result<PathBuf, Error> {
    map_stdout(&["--bin-dir"], |s| PathBuf::from(s))
}

/// Directory containing LLVM headers.
pub fn include_dir() -> Result<PathBuf, Error> {
    map_stdout(&["--include-dir"], |s| PathBuf::from(s))
}

/// Directory containing LLVM libraries.
pub fn lib_dir() -> Result<PathBuf, Error> {
    map_stdout(&["--lib-dir"], |s| PathBuf::from(s))
}

/// Directory containing LLVM cmake modules.
pub fn cmake_dir() -> Result<PathBuf, Error> {
    map_stdout(&["--cmake-dir"], |s| PathBuf::from(s))
}

/// C preprocessor flags for files that include LLVM headers.
pub fn cpp_flags() -> Result<impl Iterator<Item = String>, Error> {
    stdout_words(&["--cppflags"])
}

/// C compiler flags for files that include LLVM headers.
pub fn c_flags() -> Result<impl Iterator<Item = String>, Error> {
    stdout_words(&["--cflags"])
}

/// C++ compiler flags for files that include LLVM headers.
pub fn cxx_flags() -> Result<impl Iterator<Item = String>, Error> {
    stdout_words(&["--cxxflags"])
}

/// Print Linker flags.
pub fn ldflags() -> Result<impl Iterator<Item = String>, Error> {
    stdout_words(&["--ldflags"])
}

/// System Libraries needed to link against LLVM components.
pub fn system_libs() -> Result<impl Iterator<Item = String>, Error> {
    stdout_words(&["--system-libs"])
}

/// Libraries needed to link against LLVM components.
pub fn libs() -> Result<impl Iterator<Item = String>, Error> {
    stdout_words(&["--libs"])
}

/// Bare library names for in-tree builds.
pub fn libnames() -> Result<String, Error> {
    map_stdout(&["--libnames"], |s| String::from(s))
}

/// Fully qualified library filenames for makefile depends.
pub fn libfiles() -> Result<impl Iterator<Item = String>, Error> {
    stdout_words(&["--libfiles"])
}

/// List of all possible components.
pub fn components() -> Result<impl Iterator<Item = String>, Error> {
    stdout_words(&["--components"])
}

#[derive(Debug)]
struct SpaceSeparatedStrings {
    src: String,
    next_character_index: usize,
}

impl SpaceSeparatedStrings {
    fn new<S: Into<String>>(src: S) -> Self {
        SpaceSeparatedStrings {
            src: src.into(),
            next_character_index: 0,
        }
    }
}

impl Iterator for SpaceSeparatedStrings {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let rest = &self.src[self.next_character_index..];
        let trimmed = rest.trim_start();
        // Note: We need to keep track of how much whitespace was skipped...
        self.next_character_index += rest.len() - trimmed.len();
        let rest = trimmed;

        if rest.is_empty() {
            return None;
        }

        let word = match rest.find(char::is_whitespace) {
            Some(end_ix) => &rest[..end_ix],
            None => rest,
        };

        self.next_character_index += word.len();
        Some(word.to_string())
    }
}

fn run<I, O>(args: I) -> Result<Output, Error>
where
    I: IntoIterator<Item = O>,
    O: AsRef<OsStr>,
{
    let mut command = Command::new("llvm-config");
    command.stdin(Stdio::null());

    for arg in args {
        command.arg(arg);
    }

    let output = command.output().map_err(Error::UnableToInvoke)?;

    if output.status.success() {
        Ok(output)
    } else {
        Err(Error::BadExitCode(output))
    }
}

/// Invoke `llvm-config` then transform STDOUT.
fn map_stdout<I, O, F, T>(args: I, map: F) -> Result<T, Error>
where
    I: IntoIterator<Item = O>,
    O: AsRef<OsStr>,
    F: FnOnce(&str) -> T,
{
    let output = run(args)?;
    let stdout = String::from_utf8(output.stdout)?;
    Ok(map(stdout.trim()))
}

/// Invoke `llvm-config` then split STDOUT by spaces.
fn stdout_words<I, O>(args: I) -> Result<impl Iterator<Item = String>, Error>
where
    I: IntoIterator<Item = O>,
    O: AsRef<OsStr>,
{
    let output = run(args)?;
    let stdout = String::from_utf8(output.stdout)?;
    Ok(SpaceSeparatedStrings::new(stdout))
}

/// An error that may occur while trying to use `llvm-config`.
#[derive(Debug)]
pub enum Error {
    /// The output wasn't valid UTF-8.
    Utf8(FromUtf8Error),
    /// Unable to invoke `llvm-config`.
    UnableToInvoke(io::Error),
    /// The command ran to completion, but finished with an unsuccessful status
    /// code (as reported by [`std::process::ExitStatus`]).
    BadExitCode(Output),
}

impl From<FromUtf8Error> for Error {
    fn from(other: FromUtf8Error) -> Error { Error::Utf8(other) }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::Utf8(_) => write!(f, "The output wasn't valid UTF-8"),
            Error::UnableToInvoke(_) => write!(f, "Unable to invoke llvm-config. Is it installed and on your $PATH?"),
            Error::BadExitCode(output) => {
                write!(f, "llvm-config ran unsuccessfully")?;

                if let Some(code) = output.status.code() {
                    write!(f, " with exit code {}", code)?;
                }

                Ok(())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Utf8(inner) => Some(inner),
            Error::UnableToInvoke(inner) => Some(inner),
            Error::BadExitCode(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_split_correctly() {
        let src = "aarch64 aarch64asmparser aarch64codegen aarch64desc
        aarch64disassembler aarch64info aarch64utils aggressiveinstcombine
        all all-targets amdgpu amdgpuasmparser amdgpucodegen";
        let expected = vec![
            "aarch64",
            "aarch64asmparser",
            "aarch64codegen",
            "aarch64desc",
            "aarch64disassembler",
            "aarch64info",
            "aarch64utils",
            "aggressiveinstcombine",
            "all",
            "all-targets",
            "amdgpu",
            "amdgpuasmparser",
            "amdgpucodegen",
        ];

        let got: Vec<_> = SpaceSeparatedStrings::new(src).collect();

        assert_eq!(got, expected);
    }
}
