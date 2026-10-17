use nix::unistd::{User, getuid};

use super::user_home;

#[test]
fn without_the_override_the_home_is_in_the_home_folder_the_user_database_gives_this_uid() {
    let account = User::from_uid(getuid()).unwrap().unwrap();
    assert_eq!(user_home().unwrap(), account.dir.join(".agent-terminal"));
}
