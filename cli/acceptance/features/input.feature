Feature: Driving a program's input
  Scenario: typed text reaches the program
    Given a terminal running a program that echoes its input
    When the agent types "hello"
    Then the command succeeds
    And the program received "hello"

  Scenario: named keys reach the program as the bytes a terminal sends
    Given a terminal running a program that echoes its input
    When the agent presses "Up" then "Enter"
    Then the command succeeds
    And the program received "\e[A\r"

  Scenario: raw bytes reach the program exactly
    Given a terminal running a program that echoes its input
    When the agent sends the bytes "a\x01\t"
    Then the command succeeds
    And the program received "a\x01\t"
