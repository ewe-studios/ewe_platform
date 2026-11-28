# Set up test peers and views

Run these commands from the repository root.

```sh
cargo test --locked --profile uat -p foundation_deployment_platform --test docker_macro_tests --test docker_macro_clock_arg_tests
```

```sh
cargo test --locked --profile uat -p foundation_wireguard --features spec55-complete --test completeness_tests macro_produces_valid_config
```

```sh
cargo test --locked --profile uat -p foundation_proxy --test proxy_macro_tests
```

```sh
cargo test --locked --profile uat -p foundation_wasm_ui --test html_macro_tests
```

```sh
cargo test --locked --profile uat -p foundation_wasm_ui --test theme_tests
```

The following fixtures give the setup and assertions.

- [Serve test traffic through container groups](../../../foundation_deployment_platform/tests/docker_macro_tests.rs)
- [Inject a clock into container setup](../../../foundation_deployment_platform/tests/docker_macro_clock_arg_tests.rs)
- [Build mesh configuration](../../../foundation_wireguard/tests/completeness_tests.rs)
- [Compare proxy settings with the builder](../../../foundation_proxy/tests/proxy_macro_tests.rs)
- [Render and update elements in a view](../../../foundation_wasm_ui/tests/html_macro_tests.rs)
- [Derive theme tokens and apply them to an app](../../../foundation_wasm_ui/tests/theme_tests.rs)

The live container examples need a running Docker service and its configured test images.
