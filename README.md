# ZCB Live

A free live Geometry Dash clickbot.

THX spinningtoilet

# Building

There is a `build.py` file which will run cargo build (--release), copy the built DLL into the GD modloader folder
and start GeometryDash.exe. To use it, put this into a file called `gamepath.txt`:

```
C:\Games\Geometry Dash\GDMenu\dll
C:\Games\Geometry Dash\
debug
```

The first line is your DLL modloader folder, second line is the path with GeometryDash.exe, and the third line
is `debug` or `release` (changes cargo build mode).

ALWAYS BUILD IN RELEASE MODE!!

You can also start the `build.py` file by double-clicking `build.bat`
