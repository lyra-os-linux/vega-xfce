use std::io;
use std::process::{Command, Stdio};

/// Módulos fornecidos pelo próprio XFCE. O Vega os organiza em uma central,
/// mas as preferências continuam sendo persistidas nativamente no xfconf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Module {
    Appearance,
    Windows,
    Desktop,
    Panel,
    Screensaver,
    Power,
    Settings,
}

impl Module {
    fn command(self) -> (&'static str, &'static [&'static str]) {
        match self {
            Self::Appearance => ("xfce4-appearance-settings", &[]),
            Self::Windows => ("xfwm4-settings", &[]),
            Self::Desktop => ("xfdesktop-settings", &[]),
            Self::Panel => ("xfce4-panel", &["--preferences"]),
            Self::Screensaver => ("xfce4-screensaver-preferences", &[]),
            Self::Power => ("xfce4-power-manager-settings", &[]),
            Self::Settings => ("xfce4-settings-manager", &[]),
        }
    }
}

pub fn open_module(module: Module) -> io::Result<()> {
    let (program, args) = module.command();
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_modules_have_expected_commands() {
        assert_eq!(
            Module::Appearance.command(),
            ("xfce4-appearance-settings", &[][..])
        );
        assert_eq!(Module::Desktop.command(), ("xfdesktop-settings", &[][..]));
        assert_eq!(
            Module::Panel.command(),
            ("xfce4-panel", &["--preferences"][..])
        );
        assert_eq!(
            Module::Power.command(),
            ("xfce4-power-manager-settings", &[][..])
        );
        assert_eq!(
            Module::Screensaver.command(),
            ("xfce4-screensaver-preferences", &[][..])
        );
    }
}
