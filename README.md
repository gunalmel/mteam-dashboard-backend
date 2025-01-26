Build the entire workspace:

```shell
cargo build
```

* The target directory will be created in the project root, containing build artifacts for each package.
* The target/debug directory will contain the executable files for each package.
* The target/debug/deps directory will contain the dependency libraries.

Run the web package:

```shell
cargo run -p mteam-dashboard-backend
```

Build a specific package:

```shell
cargo build -p mteam-dashboard-action-processor
```

Run tests for the entire workspace:

```shell
cargo test
```

Run tests for a specific package:

```shell
cargo test -p mteam-dashboard-plotly-processor
```
