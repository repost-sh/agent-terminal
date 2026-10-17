Feature: Sessions and their tabs
  Scenario: every open session is listed
    Given terminals open in the sessions "work" and "play"
    When the agent lists the sessions
    Then the command succeeds
    And the list names exactly the sessions "play" and "work"

  Scenario: closing a session's last tab keeps the session
    Given terminals open in the sessions "work" and "play"
    When the agent closes the tab in "work"
    And the agent lists the sessions
    Then the command succeeds
    And the list names exactly the sessions "play" and "work"

  Scenario: closing every tab of a session ends the session
    Given terminals open in the sessions "work" and "play"
    When the agent closes every tab in "work"
    And the agent lists the sessions
    Then the command succeeds
    And the list names exactly the sessions "play"

  Scenario: a program opened without naming a session is refused
    When the agent opens a program without naming a session
    Then the command fails with exit code 1
    And the error says to create the session first with "agent-terminal session new"

  Scenario: a program opened without naming its tab is refused
    Given the agent works in its own session "qa"
    When the agent opens a program without naming its tab
    Then the command fails with exit code 1
    And the error says to name the tab with "--name api"

  Scenario: a tab is reached by its name, and by its new name after a rename
    Given the agent works in its own session "qa"
    And tabs named "api" and "app" are open
    When the agent renames the tab "api" to "web"
    And the agent reads the screen of the tab "web"
    Then the command succeeds
    And the screen is the tab "web"
