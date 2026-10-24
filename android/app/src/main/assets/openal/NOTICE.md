# OpenAL Soft

This application bundles a dynamically loaded OpenAL Soft 1.25.2 loopback library.
The license texts in this directory accompany OpenAL Soft and its bundled pffft,
fmt and GSL components. See their source headers for copyright notices.

Unmodified source archive:
https://codeload.github.com/kcat/openal-soft/zip/refs/tags/1.25.2

SHA-256: 47e22c066dffa2f65a1747272978344db845dcef9c7ee118449ec560a44adc8f

Build recipe: scripts/Build-OpenALAndroid.ps1 in this project's sources.
NDK 27.2.12479018; API 26; arm64-v8a / x86_64; static C++ runtime;
16 KiB ELF alignment; C++20 modules, EAX, examples, utilities, embedded HRTF data
and hardware backends disabled. No upstream source patches are applied.
The library is separate from librc_native.so and can be rebuilt and replaced
when rebuilding this APK from the available project sources.
