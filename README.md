## Makefile but in Rust.

## From now on, you can see how i develop this project on my twitch channel: https://www.twitch.tv/rakivo8.

## See this Rakefile for example:
```Makefile
all: hello build_dir build/foo build/bar

hello:
    echo hello from Rakefile!

build_dir: build
    mkdir -p build

build/foo: src/foo.c
    cc -o $t $d

build/bar: src/bar.c src/bar.h
    cc -o $t $d[0]

.ALWAYS: hello
```

## Quickstart:
```console
$ cd examples
$ cargo run
```
