---
name: Bug report
about: Create a report to help us improve
title: ''
labels: bug
assignees: ''

---

## Describe the bug

A clear and concise description of what the bug is.

## Environment (please complete the following information):

 - OS: [e.g. Linux X11, Linux Wayland, macOS, Windows]
 - Version: please run `wezterm -V` and include its output here

## To Reproduce

Steps to reproduce the behavior.

Please include as much information as possible that can help to reproduce and
understand the issue; some pointers and suggestions are included here in this
template. You are empowered to include more or less information than is asked
for here!

## Configuration

Be sure to include the relevant section(s) of your `wezterm.lua` configuration file.

## Expected behavior
A clear and concise description of what you expected to happen.

## Screenshots

If applicable, add screenshots to help explain your problem.  Screenshots are most
appropriate for rendering issues.

## Session Recording

If the issue is with the way that escape sequences are processed it can be helpful
to capture the terminal output using the [`wt-record`](https://github.com/wez/wezterm/blob/master/wt-record)
script to run `wezterm` and record a transcript.  This requires the `script` utility
to be installed on your system (this is part of macOS and available in the `util-linux`
package on linux systems).

In the example below a file named `20180225161026.tgz` is produced.  Please attach that
file to this issue, or if it contains private or sensitive issue that you don't want the
public to see on GitHub, please find some other way to get that file to a project
contributor (perhaps Dropbox or email?).

```
$ ./wt-record
Transcript recorded in 20180225161026.tgz
```

You can use `wt-replay 20180225161026.tgz` to replay that file.

`wt-record` can only record the terminal output; it cannot record the input events going
in to the terminal, so if you are having an issue with input, please be sure to describe
it below!

## Additional context

Add any other context about the problem here.
