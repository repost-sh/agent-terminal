Feature: Reading a terminal's screen
  Scenario: an agent reads what a program printed
    Given the agent works in its own session "qa"
    And a terminal running a program that prints 3 lines
    When the agent takes a snapshot
    Then the command succeeds
    And the screen's first rows are "line 1", "line 2" and "line 3"

  Scenario: an agent waits for text that appears later
    Given the agent works in its own session "qa"
    And a terminal running a program that prints 3 lines 200 ms apart
    When the agent waits for "line 3"
    Then the command succeeds
    And the screen shows "line 3"

  Scenario: a wait for text that never appears times out
    Given the agent works in its own session "qa"
    And a terminal running a program that prints 3 lines
    When the agent waits 300 ms for "line 9"
    Then the command fails with exit code 3
    And the error names the text it waited for, "line 9"
