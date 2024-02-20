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
    if buildmode == "geode":
        target_dir = "release/"
    special = f.readline().strip() == "yes"
    
    if buildmode == "debug":
        cmd = "cargo build"
        if special:
            cmd += " --features special"
        with subprocess.Popen(cmd) as pop:
            pop.wait()
    elif buildmode == "release":
        cmd = "cargo build --release"
        if special:
            cmd += " --features special"
        with subprocess.Popen(cmd) as pop:
            pop.wait()
    elif buildmode == "geode":
        cmd = "cargo build --release --features geode"
        if special:
            cmd += ',special'
        with subprocess.Popen(cmd) as pop:
            pop.wait()

    if buildmode == "geode":
        geode_path = f.readline().strip()
        geode_proj_path = f.readline().strip()
        geode_dll_path = f.readline().strip()
        if not geode_path.endswith("/") or not geode_path.endswith("\\"):
            geode_path += "/"
        if not geode_dll_path.endswith("/") or not geode_dll_path.endswith("\\"):
            geode_dll_path += "/"
        
        # meow
        shutil.copyfile(TARGET + target_dir + DLLNAME, geode_dll_path + DLLNAME)
        print('bundling live.dll...')
        os.system(f"cd \"{geode_proj_path}\" && python bundle.py")
        print('building geode project...')
        os.system(f"cd \"{geode_proj_path}\" && cmake --build build --config Release")
        print('starting gd...')
        os.system(f"cd \"{geode_path}\" && GeometryDash.exe")
    else:
        # copy file to modloader folder
        shutil.copyfile(TARGET + target_dir + DLLNAME, dllpath + DLLNAME)

        # start game
        os.system("cd \"" + exe + "\" && GeometryDash.exe")
