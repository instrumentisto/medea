Contribution Guide
===

## Prerequisites

In addition to default stable [Rust] toolchain you will need `rustfmt` and `clippy` 
components, and also a nightly [Rust] toolchain (for better tooling).
```bash
rustup toolchain install nightly
rustup component add rustfmt
rustup component add clippy
```

Also you need install [wasm-pack] for [jason] building and testing:
```bash
$ curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sudo sh
```




## Operations

Take a look at [`Makefile`] for commands usage details.


### Development environment

Boot up dockerized environment for [medea] with [jason]:
```bash
$ make up.dev
```

Boot up only [medea] without [jason]:
```bash
$ make up.medea
```


### Building

To build/rebuild project and its Docker image use docker-wrapped command from [`Makefile`]:
```bash
$ make build dockerized=yes
```

To build only [medea]:
```bash
$ make build.medea
```

To build only [jason]:
```bash
$ make build.jason
```

To build [medea] in docker (it works with [jason] too):
```bash
$ make build.medea dockerized=yes
```


### Formatting

To auto-format Rust sources use command from [`Makefile`]:
```bash
$ make fmt
```


### Linting

To lint Rust sources use command from [`Makefile`]:
```bash
$ make lint
```


### Testing

To run unit tests command from [`Makefile`]:
```bash
$ make test.unit crate=@all

# or for concrete crate only
$ make test.unit crate=medea
$ make test.unit crate=jason
```

To run E2E tests use docker-wrapped commands from [`Makefile`]:
```bash
$ make test.e2e
```


### Documentation

To generate Rust sources project documentation use command from [`Makefile`]:
```bash
$ make docs.rust

# if you don't wish to automatically open docs in browser
$ make docs.rust open=no

# or for concrete crate only
$ make docs.rust crate=jason
```




## Running CI

Add `[run ci]` to your commit message.




[`Makefile`]: https://github.com/instrumentisto/medea/blob/master/Makefile
[jason]: https://github.com/instrumentisto/medea/tree/master/jason
[medea]: https://github.com/instrumentisto/medea
[Rust]: https://www.rust-lang.org/
[wasm-pack]: https://github.com/rustwasm/wasm-pack
