# rerun tool

A simple tool to run a program infinitely (reruns on close).

This tool follows this logic:

1. Start the target program.
2. If the program is closed / crashed then restart it.

Additionally, this tool performs monitoring of the files/dir passed to it and, if any is changed, restarts the the program (sending SIGTERM).

## cli interface


**1.** Just run the program forever (until CTRL+C is pressed)

```sh
./rerun <program> [arg1] [arg2] [...]
```

For example:
```sh
./rerun echo "+1"
```

**2.** Run the program forever, but restart if any of files is changes:

```sh
./rerun <file1> <file2> <...> -- <program> [arg1] [arg2] [...]
```

For example:
```sh
./rerun 1.txt -- tail -f 1.txt
```

**NOTE:** rerun always monitor the changes of the file of target program, and if it changes - restarts the program.