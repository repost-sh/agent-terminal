Feature: Sessions and their tabs
  Scenario: every open session is listed
    Given terminals open in the sessions "work" and "play"
    When the agent lists the sessions
    Then the command succeeds
    And the list names exactly the sessions "play" and "work"

  Scenario: closing a session's only tab ends the session
    Given terminals open in the sessions "work" and "play"
    When the agent closes the tab in "work"
    And the agent lists the sessions
    Then the command succeeds
    And the list names exactly the sessions "play"
