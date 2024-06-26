## Makefile but in Rust.

## From now on, you can see how i develop this project on my twitch channel: https://www.twitch.tv/rakivo8.

## See this Rakefile for example:
```Makefile
# This is how you can declare your variables:
cc = cc
cflags = -O0 -w
name = rakivo
build = build

# You can use your variables just like in Makefile
all: c_test $(name) print_target print_deps test_silent

c_test: src/foo.c
    cc $(cflags) -o build/$@ $d

$(name):
    echo hello $(name)!

# To get target of your job you can use `$t` syntax, or just `$@`", like in Makefile :)
print_target:
    echo target is $t
    echo target is also $@
    printf '\n'

# To get your first dependency you can use `$d` or `$<` syntax,
# to get all of the dependencies you can use `$ds` or `$^`.
print_deps: src/bar.c src/bar.h
    echo deps is $ds
    echo deps is also $^
    printf '\n'

# You can also index your dependencies:
    echo deps[1] is $d[1]

# We also have special targets like: `.PHONY`, `.SILENT`, ...
# `.ALWAYS` is basically an analog of the `.PHONY`.
.ALWAYS: hello test_silent

# Let's test `.SILENT`:
test_silent:
    touch hello.txt
    echo "hello from Rakefile" > hello.txt

# And make test_silent silent
.SILENT: test_silent

# ```
# touch hello.txt
# echo "hello from Rakefile" > hello.txt
# ```
# Shouldn't be printed
```

## Quickstart:
```console
$ cargo run --release -- -C ./examples/
```
