# Fluke

> [!NOTE]
> This is a tool developed for internal use, not an official Antithesis product.
> That said, we thought it was cool enough to release, and we hope you can use it (or at least take some ideas from it)!

Fluke is a tracing-based Nix evaluation cacher, which tracks a given evaluation's impurities, then looks for changes to them upon subsequent evals (primarily by using [Watchman](https://facebook.github.io/watchman/)).

## Usage

The Nix expression at `default.nix` contains everything you need to install Fluke:

```console
$ nix-build
[...]
$ ./result/bin/fluke
```

Fluke has two entrypoints: `fluke` and `fluke-nix-build`. The latter is a wrapper around the former, providing a `nix-build`-like interface for adapting existing usages of the Nix CLI.

## Limitations

- Currently, Fluke only supports evaluating attrsets. This will probably change in the future!
- Our impurity logging patch only supports Lix, therefore Fluke only supports evaluating using Lix.

### Why a patch to Nix?

Nix (and Lix) unfortunately don't give us information about all impurities used by an evaluation, so we do The Easy Thing and give Lix a way to emit structured impurity metadata.

## Acknowledgements

The general idea behind Fluke was originally implemented by Dave Scherer.
