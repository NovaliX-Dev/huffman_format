use std::{io::{self, IsTerminal}, process::ExitCode};

use anyhow::Context;
use clap::Parser;
use cli::Cli;
use ::log::{error, info};

mod cli {
    use std::{ffi::OsString, fs::File, io::{self, IsTerminal, Read, Seek, StdinLock, StdoutLock, Write}, path::PathBuf};

    use derive_more::Display;
    use log::warn;

    #[derive(Debug, thiserror::Error, PartialEq, Eq)]
    pub enum ValidationError {
        #[error("Can't pack with stdin as input.")]
        CannotPackWithStdinAsInput,

        #[error("The output file must be specified when using stdin as input.")]
        RequiresOutputWhenUsingStdin
    }

    #[derive(clap::Parser, Debug)]
    pub struct Cli {
        pub command: Command,

        #[clap(value_parser = Input::parse_value)]
        input: Input,

        #[clap(short, long, value_parser = Output::parse_value)]
        output: Option<Output>,

        #[clap(short='W', long)]
        pub overwrite: bool
    }
    
    impl Cli {
        pub fn validate_input(&self) -> Result<&Input, ValidationError> {
            if matches!(self.command, Command::Pack) && matches!(self.input, Input::Stdin) {
                return Err(ValidationError::CannotPackWithStdinAsInput)
            }

            Ok(&self.input)
        }
        
        pub fn validate_output(&self) -> Result<Output, ValidationError> {
            if let Some(output) = &self.output {
                return Ok(output.clone())
            }

            fn add_extension(path: &mut PathBuf, part: &str) -> OsString{
                let mut extension = path.extension().unwrap_or_default().to_owned();
                if !extension.is_empty() {
                    extension.push(".");
                }
                extension.push(part);

                path.set_extension(&extension);

                extension
            }

            if let Input::File(input_path) = &self.input {
                let extension = input_path.extension();
                let mut path = input_path.to_owned();

                let path = match self.command {
                    Command::Pack => {
                        add_extension(&mut path, "hc");
                        path
                    }
                    Command::Unpack => if extension.is_some_and(|ext| ext == "hc") {
                        path.set_extension("");
                        path
                    } else {
                        let new_extension = add_extension(&mut path, "unpacked");
    
                        warn!("The input file doesn't have the extension `hc`. The output file extension be `{}`", new_extension.to_string_lossy());
    
                        path
                    }
                };

                return Ok(Output::File(path))
            }

            if !io::stdout().is_terminal() {
                Ok(Output::Stdout)
            } else {
                Err(ValidationError::RequiresOutputWhenUsingStdin)
            }
        }
    }

    #[derive(Clone, Debug, Display, PartialEq, Eq)]
    pub enum Input {
        #[display("<stdin>")]
        Stdin,

        #[display("{}", _0.display())]
        File(PathBuf),
    }

    impl Input {
        fn parse_value(str: &str) -> Result<Self, String> {
            if str.trim() == "-" {
                return Ok(Self::Stdin)
            }
            
            let path = PathBuf::from(str);

            if !path.exists() {
                return Err("Expected the input file to exists.".to_string())
            }
            if !path.is_file() {
                return Err("Expected the input path to be a file.".to_string())
            }

            Ok(Self::File(path))
        }

        pub fn open(&self) -> io::Result<InputRead> {
            match self {
                Self::Stdin => {
                    if io::stdin().is_terminal() {
                        warn!("There are no pipes which the program reads from. The result will be empty.");
                        return Ok(InputRead::Empty)
                    }

                    Ok(InputRead::Stdin(io::stdin().lock()))
                }
                Self::File(path) => {
                    let file = File::open(path)?;
                    Ok(InputRead::File(file))
                }
            }
        }
    }

    pub enum InputRead {
        Stdin(StdinLock<'static>),
        File(File),
        Empty
    }

    impl Read for InputRead {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self {
                Self::Stdin(stdin) => stdin.read(buf),
                Self::File(file) => file.read(buf),
                Self::Empty => Ok(0)
            }
        }
    }

    impl Seek for InputRead {
        fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
            match self {
                Self::Stdin(_) => panic!("Can't seek on stdin"),
                Self::File(file) => file.seek(pos),
                Self::Empty => Ok(0)
            }
        }
    }


    #[derive(Clone, Debug, Display, PartialEq, Eq)]
    pub enum Output {
        #[display("<stdout>")]
        Stdout,
        #[display("{}", _0.display())]
        File(PathBuf)
    }

    impl Output {
        fn parse_value(str: &str) -> Result<Self, String> {
            if str.trim() == "-" {
                return Ok(Self::Stdout)
            }

            Ok(Self::File(PathBuf::from(str)))
        }

        pub fn open(&self, overwrite: bool) -> io::Result<OutputWrite> {
            match self {
                Self::Stdout => {
                    Ok(OutputWrite::Stdout(io::stdout().lock()))
                }

                Self::File(path) => {
                    let file = if overwrite { 
                        File::create(path)? 
                    } else { 
                        File::create_new(path)? 
                    };

                    Ok(OutputWrite::File(file))
                }
            }
        }

        pub fn delete(&self) -> io::Result<()> {
            match self {
                Self::File(path) => {
                    std::fs::remove_file(path)
                }

                _ => Ok(())
            }
        }
    }

    pub enum OutputWrite {
        Stdout(StdoutLock<'static>),
        File(File)
    }

    impl Write for OutputWrite {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            match self {
                Self::File(file) => file.write(buf),
                Self::Stdout(stdout) => stdout.write(buf)
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            match self {
                Self::File(file) => file.flush(),
                Self::Stdout(stdout) => stdout.flush()
            }
        }
    }

    #[derive(clap::ValueEnum, Clone, Debug)]
    pub enum Command {
        Pack,
        Unpack
    }
}

mod log {
    use colog::format::CologStyle;
    use env_logger::fmt::Formatter;
    use log::{Level, Record};
    use once_cell::race::OnceBool;

    struct ColorFormatter;

    impl CologStyle for ColorFormatter {
        fn level_token(&self, level: &Level) -> &str {
            match level {
                Level::Error => "Error",
                Level::Warn => "Warning",
                Level::Info => "Info",
                Level::Debug => "Debug",
                Level::Trace => "Trace",
            }
        }
        fn prefix_token(&self, level: &Level) -> String {
            self.level_color(level, &format!("{: >7}", self.level_token(level)))
        }
        fn line_separator(&self) -> String {
            "\n".to_string() + &" ".repeat(7)
        }
    }

    static ACTIVE: OnceBool = OnceBool::new();

    fn custom_format(buf: &mut Formatter, record: &Record<'_>) -> Result<(), std::io::Error> {
        if ACTIVE.get().unwrap() {
            let color_formatter = ColorFormatter;
            color_formatter.format(buf, record)
        } else {
            Ok(())
        }
    }

    pub fn init(active: bool) {
        ACTIVE.set(active).unwrap();

        colog::basic_builder()
            .format(custom_format)
            .filter_level(log::LevelFilter::Info)
            .init();
    }
}

fn try_main(cli: Cli) -> anyhow::Result<()> {
    let input = cli.validate_input()?;
    let output = cli.validate_output()?;
    
    info!("Opening `{}`...", input);

    let mut input_read = input.open().with_context(|| "Failed to open the input file")?;
    
    info!("Writing to `{}`...", output);
    let mut output_write = output.open(cli.overwrite).with_context(|| "Failed to create the output file")?;

    let res = match cli.command {
        cli::Command::Pack => {
            huffman_format::pack_file(&mut input_read, &mut output_write)
                .with_context(|| "Failed to pack the input file")
        },
        cli::Command::Unpack => {
            huffman_format::unpack_file(&mut input_read, &mut output_write)
                .with_context(|| "Failed to unpack the data")
        },
    };
    if let Err(err) = res {
        error!("{:#}", err);

        if !io::stdout().is_terminal() {
            eprintln!("Error : {:#}", err);
        }
        
        output.delete().with_context(|| "Failed to remove the output file")?
    }

    Ok(())
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    log::init(io::stdout().is_terminal());

    if let Err(err) = try_main(cli) {
        error!("{:#}", err);

        if !io::stdout().is_terminal() {
            eprintln!("Error : {:#}", err);
        }
        return ExitCode::FAILURE
    }

    ExitCode::SUCCESS
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::path::PathBuf;

    use clap::Parser;

    use crate::cli::{Cli, ValidationError};

    #[test]
    fn test_clap_arguments() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn clap_refuses_stdin_when_packing() {
        let cli = Cli::try_parse_from(["", "pack", "-", "-o", "-"]).unwrap();
        assert_eq!(cli.validate_input(), Err(ValidationError::CannotPackWithStdinAsInput));
    }

    #[test]
    fn clap_unpack_requires_output_when_using_stdin_as_input() {
        let cli = Cli::try_parse_from(["", "unpack", "-"]).unwrap();
        assert_eq!(cli.validate_output(), Err(ValidationError::RequiresOutputWhenUsingStdin));
    }

    macro_rules! create_temp_files {
        ($($files_names: literal => $path_str: ident),* in $temp_dir: ident) => {
            let $temp_dir = tempfile::tempdir().unwrap();

            $(std::fs::File::create_new($temp_dir.path().join($files_names)).unwrap();)*

            $(let $path_str = $temp_dir.path().join($files_names).display().to_string();)*
        };
    }

    #[test]
    fn clap_accept_paths_correctly() {
        create_temp_files!("a" => a_path_str, "b" => b_path_str in temp_dir);

        let cli = Cli::try_parse_from(["", "unpack", &a_path_str, "-o", &b_path_str]).unwrap();
        assert_eq!(cli.validate_input(), Ok(&crate::cli::Input::File(PathBuf::from(&a_path_str))));
        assert_eq!(cli.validate_output(), Ok(crate::cli::Output::File(PathBuf::from(&b_path_str))));

        let cli = Cli::try_parse_from(["", "pack", &a_path_str, "-o", &b_path_str]).unwrap();
        assert_eq!(cli.validate_input(), Ok(&crate::cli::Input::File(PathBuf::from(&a_path_str))));
        assert_eq!(cli.validate_output(), Ok(crate::cli::Output::File(PathBuf::from(&b_path_str))));
    }

    #[test]
    fn test_unpack_output_path_is_deduced_correctly_from_input_path_when_not_provided() {
        create_temp_files!("a.hc" => a_path_str, "a" => a2_path_str, "a.extension" => a_with_extension_path_str in temp_dir);

        let cli = Cli::try_parse_from(["", "unpack", &a_path_str]).unwrap();
        assert_eq!(cli.validate_input(), Ok(&crate::cli::Input::File(PathBuf::from(&a_path_str))));
        assert_eq!(cli.validate_output(), Ok(crate::cli::Output::File(PathBuf::from(temp_dir.path().join("a")))));

        let cli = Cli::try_parse_from(["", "unpack", &a2_path_str]).unwrap();
        assert_eq!(cli.validate_input(), Ok(&crate::cli::Input::File(PathBuf::from(&a2_path_str))));
        assert_eq!(cli.validate_output(), Ok(crate::cli::Output::File(PathBuf::from(temp_dir.path().join("a.unpacked")))));

        let cli = Cli::try_parse_from(["", "unpack", &a_with_extension_path_str]).unwrap();
        assert_eq!(cli.validate_input(), Ok(&crate::cli::Input::File(PathBuf::from(&a_with_extension_path_str))));
        assert_eq!(cli.validate_output(), Ok(crate::cli::Output::File(PathBuf::from(temp_dir.path().join(a_with_extension_path_str + ".unpacked")))));
    }

    #[test]
    fn testpack_output_path_is_deduced_correctly_from_input_path_when_not_provided() {
        create_temp_files!("a" => a_path_str in temp_dir);

        let cli = Cli::try_parse_from(["", "pack", &a_path_str]).unwrap();
        assert_eq!(cli.validate_input(), Ok(&crate::cli::Input::File(PathBuf::from(&a_path_str))));
        assert_eq!(cli.validate_output(), Ok(crate::cli::Output::File(PathBuf::from(temp_dir.path().join("a.hc")))));
    }
}
