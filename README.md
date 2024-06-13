## Makefile but in Rust.

## From now on, you can see how i develop this project on my twitch channel: https://www.twitch.tv/rakivo8.

## See this Rakefile for example:
```Makefile
all: hello build/foo build/bar

hello:
    echo hello from Rakefile!

build/foo: src/foo.c
    mkdir -p build
    cc -o $t $d

build/bar: src/bar.c src/bar.h
    mkdir -p build
    cc -o $t $d

.ALWAYS: hello
```

## Quickstart:
```console
$ cd examples
$ cargo run
```
