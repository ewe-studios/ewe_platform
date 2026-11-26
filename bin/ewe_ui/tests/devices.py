"""Check selected devices, startup errors, and cancellation through the installed CLI."""
from pathlib import Path
import os
import signal
import subprocess
import sys
import tempfile
import time

binary = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    sdk = root / "sdk"
    def tool(file, source):
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text("#!" + sys.executable + "\n" + source)
        file.chmod(0o755)
    tool(sdk/"platform-tools/adb", """
import os,sys
from pathlib import Path
if sys.argv[1:] == ['devices']:
    print('List of devices attached')
    if os.environ.get('CONNECTED'):
        print('emulator-1234\\tdevice')
elif 'emu' in sys.argv: print('DesignPhone')
elif 'getprop' in sys.argv: print('1')
else: sys.exit(9)
""")
    tool(sdk/"emulator/emulator", """
import os,sys,time
from pathlib import Path
if sys.argv[1:] == ['-list-avds']: print('DesignPhone')
else:
    Path(os.environ['PID_LOCATION']).write_text(str(os.getpid()))
    if os.environ.get('FAIL'): sys.exit(15)
    time.sleep(120)
""")
    pid_file = root/"emulator.pid"
    env = dict(os.environ,ANDROID_HOME=str(sdk),PID_LOCATION=str(pid_file))
    def cli(args, **options):
        return subprocess.run([binary,*args],cwd=root,env=options.pop("env",env),capture_output=True,
                              text=True,timeout=12,**options)
    cmd=["devices","start","--target","android","--device","DesignPhone","--poll-milliseconds","25"]
    assert "Stopped" in cli(["devices","list","--target","android"]).stdout
    assert cli(cmd,env=dict(env,CONNECTED="1")).returncode == 0
    assert not pid_file.exists()
    failure=cli(cmd,env=dict(env,FAIL="1"))
    assert failure.returncode != 0 and "15" in failure.stderr,failure
    pid_file.unlink()
    unknown=cli(["devices","start","--target","android","--device","unknown"])
    assert unknown.returncode != 0 and "unknown" in unknown.stderr
    no_target=cli(["devices","list","--target","web"])
    assert no_target.returncode != 0
    started=subprocess.Popen([binary,*cmd],cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        deadline=time.monotonic()+8
        while not pid_file.exists() and time.monotonic()<deadline: time.sleep(0.05)
        pid=int(pid_file.read_text())
        started.send_signal(signal.SIGINT)
        out,err=started.communicate(timeout=8)
        assert started.returncode == 130,(out,err)
        try: os.kill(pid,0)
        except ProcessLookupError: pass
        else: raise AssertionError("Cancellation left the emulator running")
    finally:
        if started.poll() is None: started.kill(); started.wait()
    print("Device selection, booted-device reuse, startup failure, and emulator cancellation passed.")
