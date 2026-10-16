macro_rules! in_every_shell {
    ($($scenario:ident),* $(,)?) => {
        $(mod $scenario {
            #[test]
            fn zsh() {
                super::$scenario("zsh")
            }

            #[test]
            fn bash() {
                super::$scenario("bash")
            }

            #[test]
            fn fish() {
                super::$scenario("fish")
            }
        })*
    };
}

mod command_log;
mod exec;
mod history;
mod opening;
mod tab;
