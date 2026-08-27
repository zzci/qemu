@echo off
rem Installs the guest side of virtiofs (`shares` in vmd.toml) during the unattended install:
rem WinFsp (the FUSE layer virtiofs.exe links against), the viofs driver and the VirtioFsSvc
rem service that mounts the share as a drive letter. Staged by the install script next to this
rem file, so %~dp0 holds winfsp.msi and VioFS\. Log: C:\Windows\Temp\virtiofs-setup.log
setlocal
set "SRC=%~dp0"
set "LOG=C:\Windows\Temp\virtiofs-setup.log"
set "DEST=C:\Program Files\Virtio-Win\VioFS"
set "DRIVE=Z:"
echo [%DATE% %TIME%] setup-virtiofs from %SRC% >>"%LOG%"

if exist "%SRC%winfsp.msi" (
    msiexec /i "%SRC%winfsp.msi" /qn /norestart >>"%LOG%" 2>&1
    echo winfsp msiexec exit=%ERRORLEVEL% >>"%LOG%"
) else (
    echo winfsp.msi missing - virtiofs will not work >>"%LOG%"
)

pnputil /add-driver "%SRC%VioFS\viofs.inf" /install >>"%LOG%" 2>&1
echo pnputil exit=%ERRORLEVEL% >>"%LOG%"

if not exist "%DEST%" mkdir "%DEST%"
copy /y "%SRC%VioFS\virtiofs.exe" "%DEST%\" >>"%LOG%" 2>&1

rem -m %DRIVE% is required: with no mount point (and no UNC prefix) virtiofs.exe runs but mounts
rem nothing, so the share never shows up.
rem delayed-auto, and only WinFsp.Launcher as a dependency: on the first boot after this install
rem the viofs driver service (VirtioFsDrv) does not exist yet - PnP creates it while booting, once
rem it sees the device - so a plain auto-start service dies with 1075 and the share only appears
rem after a reboot. The delayed start runs well after PnP has settled; the failure actions retry
rem in case it still lost the race (or the guest simply has no share attached).
sc create VirtioFsSvc binPath= "\"%DEST%\virtiofs.exe\" -m %DRIVE%" start= delayed-auto depend= "WinFsp.Launcher" DisplayName= "Virtio FS Service" >>"%LOG%" 2>&1
echo sc create exit=%ERRORLEVEL% >>"%LOG%"
sc failure VirtioFsSvc reset= 0 actions= restart/20000/restart/60000/restart/60000 >>"%LOG%" 2>&1
sc failureflag VirtioFsSvc 1 >>"%LOG%" 2>&1
sc start VirtioFsSvc >>"%LOG%" 2>&1
echo sc start exit=%ERRORLEVEL% >>"%LOG%"
echo [%DATE% %TIME%] setup-virtiofs done >>"%LOG%"
endlocal
