import shutil
import os
import subprocess

TARGET = "target/"
DLLNAME = "live.dll"

with open("gamepath.txt", "r", encoding = "utf-8") as f:
    dllpath = f.readline().strip() # path to dll loader folder
    if not dllpath.endswith("/") or not dllpath.endswith("\\"):
        dllpath += "/"
    exe = f.readline().strip() # path to GeometryDash.exe
    buildmode = f.readline().strip()
    target_dir = buildmode + "/"

    if buildmode == "debug":
        with subprocess.Popen("cargo build --features special") as pop:
            pop.wait()
    else:
        with subprocess.Popen("cargo build --release") as pop:
            pop.wait()

    # copy file to modloader folder
    shutil.copyfile(TARGET + target_dir + DLLNAME, dllpath + DLLNAME)

    # start game
    os.system("cd \"" + exe + "\" && GeometryDash.exe")
