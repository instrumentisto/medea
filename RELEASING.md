HOWTO: Releasing
================

All releases of this repository are normally made by [GitHub Actions] when release [Git tag][2] is pushed. Manual releasing should be avoided asap.




## BEFORE release

1. __Ensure crate's dependencies do NOT contain `path` option in `Cargo.toml`.__  
While referring local dependency via `path` is neat and helpful for development, it cannot be used for publishing a release version to [crates.io].

2. __Set the correct crate version in its `Cargo.toml`.__  
Usually, this is a change from `x.y.z-dev` to `x.y.z`. Ensure version bump follows [Semantic Versioning 2.0.0].

3. __Check release build succeeds with `cargo package`.__  
To be sure that crate will be released OK on [crates.io] run `make release.crates crate=<crate-name> publish=no` and fix any appeared errors.

4. __Correct and prepare crate's `CHANGELOG.md`.__  
All its user-facing changes should be described explicitly and clear. It should contain links to all related milestones and roadmaps of this release. It should contain the correct version and date of release.




## Realising

1. Commit all the changes for the prepared release with a commit message:
    ```
    Prepare <version> release of '<crate-name>' crate
    ```

2. Apply Git version tag of the release. Its format must be `<crate-name>-<version>`. For example:
    - `medea-macro-3.2.1` to release `3.2.1` version of `medea-macro` crate;
    - `medea-2.4.8-beta.1` to release `2.4.8-beta.1` version of `medea` crate.

3. Push the version tag to GitHub.




## After release

After release there is no need to switch crate's version back to `x.y.z-dev` and refer its local dependencies with `path` option immediately. Just do it when your development process would really require a such change.




## Broken release

If somehow the incorrect code has been released, the following steps should be done:
1. [Yank][3] the broken version of released crate on [crates.io].
2. Apply the necessary fixes to the code base.
3. Bump up patch version of the crate.
4. Prepare its release and push it.




## Manual releasing

To perform a full releasing process manually, carefully examine `Releasing` section of `.github/workflows/ci.yml` spec and repeat the necessary actions. Beware that releasing process may involve publishing not only to [crates.io], but also to [GitHub Releases][1], [NPM] and [GitHub Pages] (Helm chats, etc).





[crates.io]: https://crates.io
[GitHub Actions]: https://github.com/features/actions
[GitHub Pages]: https://pages.github.com
[Helm]: https://helm.sh
[NPM]: https://www.npmjs.com
[Semantic Versioning 2.0.0]: https://semver.org

[1]: https://help.github.com/en/articles/creating-releases
[2]: https://git-scm.com/book/en/v2/Git-Basics-Tagging
[3]: https://doc.rust-lang.org/cargo/reference/publishing.html#cargo-yank
