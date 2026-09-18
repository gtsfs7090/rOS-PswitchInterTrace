# rOS-PswitchInterTrace
An operating system process switching and interrupt tracing system


# ros-pswitchintertrace
# Overview
This project aims to show how to run the rCore operating system with process switching and interrupt tracing capabilities, so as to observe the concurrency order and event details of process switching and interrupt events, obtain conclusive evidence for conceptual understanding, and provide technical means for practical scenarios such as performance analysis and fault localization.
# Platform
Ubuntu22.04

Docker
# Steps to run the project
1. Download this project to your local machine and enter the local project folder. Command: <br>
git clone https://github.com/gtsfs7090/rOS-PswitchInterTrace.git <br>
cd rOS-PswitchInterTrace <br>
3. Build Dockerfile to generate the image ros-pswitchintertrace:1.0.0. Command: <br>
docker build -t ros-pswitchintertrace:1.0.0 . <br>
4. Execute the ros-pswitchintertrace:1.0.0 image. Command: <br>
docker run --name rostrace -it ros-pswitchintertrace:1.0.0 sh <br>
5. Enter the /home/os directory. Command: <br>
cd /home/os <br>
6. Execute rCore with process switching and interrupt tracing capabilities. Command：<br>

qemu-system-riscv64 \
    -machine virt \
    -nographic \
    -bios ../bootloader/rustsbi-qemu.bin \
-device loader,file=target/riscv64gc-unknown-none-elf/release/os.bin,addr=0x80200000

6. Result
