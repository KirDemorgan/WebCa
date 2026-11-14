use std::fmt::Write as _;

use crate::{CameraProfiles, ControlId, ControlState};

/// A value given to `webca set`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetValue {
    Value(i32),
    Auto,
}

/// A command-line request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Help,
    List,
    Get(ControlId),
    Set(ControlId, SetValue),
    Profiles,
    Apply(Option<String>),
    Mcp,
}

pub const USAGE: &str = "\
Usage: webca [command]

Without a command, WebCa opens its window.

Commands:
  list                       Show every control with its range and current value
  get <control>              Show the current value of one control
  set <control> <value|auto> Change a control right now
  profiles                   List saved profiles
  apply [profile]            Apply the active or the named profile to the camera
  mcp                        Serve the Model Context Protocol on stdin and stdout
  help                       Show this message";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UsageError {
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("`{0}` needs {1}")]
    MissingArgument(&'static str, &'static str),
    #[error("`{0}` is not a number or `auto`")]
    InvalidValue(String),
    #[error(transparent)]
    UnknownControl(#[from] crate::UnknownControl),
}

/// Parses the arguments after the program name; `None` means no command was given.
pub fn parse(arguments: &[String]) -> Option<Result<Request, UsageError>> {
    let (command, rest) = arguments.split_first()?;
    let argument = |index: usize, name: &'static str, what: &'static str| {
        rest.get(index).ok_or(UsageError::MissingArgument(name, what))
    };
    Some((|| match command.as_str() {
        "help" | "--help" | "-h" => Ok(Request::Help),
        "list" => Ok(Request::List),
        "profiles" => Ok(Request::Profiles),
        "mcp" => Ok(Request::Mcp),
        "apply" => Ok(Request::Apply(rest.first().cloned())),
        "get" => Ok(Request::Get(argument(0, "get", "a control")?.parse()?)),
        "set" => {
            let control = argument(0, "set", "a control")?.parse()?;
            let raw = argument(1, "set", "a value")?;
            let value = if raw.eq_ignore_ascii_case("auto") {
                SetValue::Auto
            } else {
                SetValue::Value(raw.parse().map_err(|_| UsageError::InvalidValue(raw.clone()))?)
            };
            Ok(Request::Set(control, value))
        }
        other => Err(UsageError::UnknownCommand(other.to_owned())),
    })())
}

/// One line per control: key, current value, mode and range.
#[must_use]
pub fn format_controls(states: &[ControlState]) -> String {
    let mut text = String::new();
    for state in states {
        let range = state.capability.range;
        let mode = if state.current.is_auto() { "auto" } else { "manual" };
        let _ = writeln!(
            text,
            "{:<24}{:>7}  {:<7} {}..{} step {} default {}",
            state.capability.id,
            state.current.value,
            mode,
            range.min(),
            range.max(),
            range.step(),
            range.default()
        );
    }
    text
}

/// Saved profile names, the active one marked with `*`.
#[must_use]
pub fn format_profiles(profiles: &CameraProfiles) -> String {
    let active = profiles.active().name();
    profiles.profiles().fold(String::new(), |mut text, profile| {
        let marker = if profile.name() == active { '*' } else { ' ' };
        let _ = writeln!(text, "{marker} {}", profile.name());
        text
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        snapshot,
        testing::{FakeCamera, sample_capabilities},
    };

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    fn run(text: &str) -> Result<Request, UsageError> {
        parse(&args(text)).expect("a command was given")
    }

    #[test]
    fn no_arguments_means_open_the_window() {
        assert_eq!(parse(&[]), None);
    }

    #[test]
    fn parses_every_command() {
        assert_eq!(run("help"), Ok(Request::Help));
        assert_eq!(run("--help"), Ok(Request::Help));
        assert_eq!(run("-h"), Ok(Request::Help));
        assert_eq!(run("list"), Ok(Request::List));
        assert_eq!(run("profiles"), Ok(Request::Profiles));
        assert_eq!(run("mcp"), Ok(Request::Mcp));
        assert_eq!(run("apply"), Ok(Request::Apply(None)));
        assert_eq!(run("apply Night"), Ok(Request::Apply(Some("Night".into()))));
        assert_eq!(run("get contrast"), Ok(Request::Get(ControlId::Contrast)));
        assert_eq!(run("set exposure auto"), Ok(Request::Set(ControlId::Exposure, SetValue::Auto)));
        assert_eq!(run("set gamma -3"), Ok(Request::Set(ControlId::Gamma, SetValue::Value(-3))));
    }

    #[test]
    fn reports_usage_errors() {
        assert_eq!(run("fly"), Err(UsageError::UnknownCommand("fly".into())));
        assert_eq!(run("get"), Err(UsageError::MissingArgument("get", "a control")));
        assert_eq!(run("set gamma"), Err(UsageError::MissingArgument("set", "a value")));
        assert_eq!(run("set"), Err(UsageError::MissingArgument("set", "a control")));
        assert_eq!(run("set gamma loud"), Err(UsageError::InvalidValue("loud".into())));
        assert_eq!(run("get iris").unwrap_err().to_string(), "unknown control `iris`");
        assert_eq!(run("set iris 3").unwrap_err().to_string(), "unknown control `iris`");
        assert_eq!(run("set gamma loud").unwrap_err().to_string(), "`loud` is not a number or `auto`");
        assert_eq!(run("get").unwrap_err().to_string(), "`get` needs a control");
        assert_eq!(run("fly").unwrap_err().to_string(), "unknown command `fly`");
        assert!(USAGE.contains("webca [command]"));
    }

    #[test]
    fn formats_controls_and_profiles() {
        let camera = FakeCamera::new(sample_capabilities());
        let mut states = snapshot(&camera).unwrap();
        states[0].current = crate::ControlSetting::auto(states[0].current.value);
        let text = format_controls(&states);
        assert_eq!(text.lines().count(), states.len());
        assert!(text.lines().next().unwrap().contains("auto"));
        assert!(text.contains("white_balance"));
        assert!(text.contains("2800..6500 step 10 default 4600"));

        let mut profiles = CameraProfiles::default();
        profiles.add("Night").unwrap();
        assert_eq!(format_profiles(&profiles), "* Default\n  Night\n");
    }
}
