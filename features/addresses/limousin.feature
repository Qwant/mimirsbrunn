Feature: Addresses
    Some scenarios for testing addresses in Limousin, France.

    Background:
        Given osm file has been downloaded for limousin
        And osm file has been processed by cosmogony for limousin
        And cosmogony file has been indexed for limousin
        And bano file has been indexed for limousin

    # With 'Exact Match', we expect the query to be found at the top of the
    # result because the query exactly matches the name / label of the target.
    Scenario Outline: Addresses exact match
        When the user searches for "<query>"
        Then he finds "<id>" as the first result

        Examples:
            | query                           | id                            |
            | 14 Place Allègre, Allassac      | addr:1.475761;45.257879:14    |
            | 1470 Rue du Puy Grasset         | addr:1.938496;45.093038:1470  |
            | 32BIS Avenue du Limousin 19230  | addr:1.385946;45.399633:32BIS |

    # When using aliases, we should still fetch the query at the top of the
    # result.
    Scenario Outline: Addresses with aliases
        When the user searches for "<query>"
        Then he finds "<id>" as the first result

        Examples:
            | query                      | id                            |
            | 14 p Allègre, Allassac     | addr:1.475761;45.257879:14    |
            | 1470 r du Puy Grasset      | addr:1.938496;45.093038:1470  |
            | 32BIS av du Limousin 19230 | addr:1.385946;45.399633:32BIS |
