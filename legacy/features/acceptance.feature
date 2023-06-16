Feature: Acceptance tests

  Scenario: Baseline scenario
    Given I have generated an index from "data.json"
    When I list all the documents in the index
    Then I find the original list

  Scenario: Double Ingestion scenario
    Given I have generated an index from "data.json"
    Given I have generated an index from "small.json"
    When I list all the documents in the index
    Then I find the original list
