# pico-rio

Measurement and control with the Raspberry Pi Pico RP 2040.

## pico-usb-ethernet

Ethernet over USB for the Raspberry Pi Pico RP 2040.

## Motivation

I have several Raspberry Pi Pico development boards based on the RP 2040.
These boards do not include Wifi or Bluetooth capabilities. The only
means for communication is the USB connection to the host computer.

The goal of this project is to experiment with UDP-over-USB to provide
a more convenient communication mechanism between the Pico and the host
computer. I want to be able to plug my Pico into my laptop and be able
to send messages and receive messages.

I have found that three requirements must be met for my use case:

- The Pico appears to the host compiler as a network interface.

- The Pico has a static IP address (arbitrarily hardcoded to 192.168.7.1)
because my laptop does not run a DHCP server.

- The Pico has to run its own DHCP server. The DHCP server assigns a
local IP address to the host computer. The host computer acts as a
gateway to the rest of the network. In particular, the Pico can
send messages to virtual machines running on the host.

## Building

1. Clone the repository.
2. Cargo currently does not support packages with different target architectures in the same workspace. You have to build the packages manually.
   a. Build the shared `messages` library:

        pushd messages
        cargo build
        popd

   b. Build the `tools`:

        pushd tools
        cargo build
        popd

   c. Build `pico-rio.uf2`:

        pushd pico
        cargo run --release
        popd

      *Note*: We build `pico-rio.uf2`, we don't install it. Edit `.cargo/config.toml` if you want to build and install it on your Raspberry Pi Pico with `probe-rs`.

## Use

Install `build/pico-usb-ethernet.uf2` on your Raspberry Pi Pico by
copying it to the Pico when then Pico is in boot mode. For example,
on macOS:

    cp build/pico-usb-ethernet.uf2 /Volumes/RPI-RP2

The Pico reboots and runs the program.

By default, the Pico registers itself under the hardcoded IP address
192.168.7.1. It echos UDP packets sent to port 12345, and sends
periodic health updates to the hardcoded IP address 192.168.64.47
and port 12345 (because that's the local address of my host computer).
Adjust these addresses and ports in [./src/main.c](./src/main.c) to match your set-up.

Use the tools in the [./tools/](./tools/) folder to talk to the Pico:

    ./tools/build/echo 192.168.7.1 12345

Sends a message to the Pico and prints the response (echo).

    ./tools/build/health 12345

Prints the health updates that the Pico sends to the host computer.
Currently, that's only the Pico's IP address and hostname.

## Attribution and License

This project is derived from Peter Lawrence's example webserver in
the TinyUSB distribution. A copy is included in the Pico SDK at

[./pico-sdk/lib/tinyusb/examples/device/net_lwip_webserver/src/main.c](./pico-sdk/lib/tinyusb/examples/device/net_lwip_webserver/src/main.c)

A copy of his original copyright notice is reproduced below.
See the LICENSE file for the licensing information of my project.

    /*
     * The MIT License (MIT)
     *
     * Copyright (c) 2020 Peter Lawrence
     *
     * influenced by lrndis https://github.com/fetisov/lrndis
     *
     * Permission is hereby granted, free of charge, to any person obtaining a copy
     * of this software and associated documentation files (the "Software"), to deal
     * in the Software without restriction, including without limitation the rights
     * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
     * copies of the Software, and to permit persons to whom the Software is
     * furnished to do so, subject to the following conditions:
     *
     * The above copyright notice and this permission notice shall be included in
     * all copies or substantial portions of the Software.
     *
     * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
     * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
     * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
     * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
     * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
     * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
     * THE SOFTWARE.
     *
     */
