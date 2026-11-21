[development]: <development.md>
[diy/0zg9/261121-durable-test-suites]: <diy/0zg9/261121-durable-test-suites/README.md>
[docs]: <../docs/README.md>

# Repository notes

The *documentation*[~][docs] in `docs/` must suffice for a clean-room rebuild of
basinsync. Material about this repository's own implementation (source layout,
vendored dependencies, test harness, and tutorials about them) lives here
instead, so clean rooms can be given `docs/` alone. Links may point into
`docs/`, never the reverse; tutorials here follow the same DAG rules and may
depend on `docs/diy`.

Build this repository with the *repository development notes*[~][development].

Keep the test suites small and resilient with
*Durable test suites*[~][diy/0zg9/261121-durable-test-suites].
