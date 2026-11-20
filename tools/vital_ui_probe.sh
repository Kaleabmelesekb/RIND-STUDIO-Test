#!/usr/bin/env bash
# Compiles Vital's interface (vendor/vital-ui) and its JUCE for Android with the NDK's clang, unit by unit, and prints
# every error: a quick check of what crates/audio/build.rs builds, without the Rust part. `tools/vital_ui_probe.sh
# arm64-v8a` (or armeabi-v7a); ANDROID_NDK_HOME names the NDK. FULL=1 generates code too (default: syntax only).
set -u
abi=${1:-arm64-v8a}
ndk=${ANDROID_NDK_HOME:?set ANDROID_NDK_HOME}
here=$(cd "$(dirname "$0")/.." && pwd)
ui=$here/vendor/vital-ui vital=$here/vendor/vital ft=$here/vendor/freetype
bin=$ndk/toolchains/llvm/prebuilt/linux-x86_64/bin
case $abi in
  arm64-v8a) triple=aarch64-linux-android26 extra="" ;;
  armeabi-v7a) triple=armv7a-linux-androideabi26 extra="-mfpu=neon -DNEON_ARM32" ;;
  *) echo "unknown abi $abi"; exit 2 ;;
esac
mode=-fsyntax-only; [ "${FULL:-0}" = 1 ] && mode=-c
defs="-DRIND_VITAL_UI=1 -DNO_AUTH=1 -DLINUX=1 -DNDEBUG=1 -DJUCE_HEADLESS_PLATFORM=1 -DOPENGL_ES=1 -DHEADLESS=0 -DJUCE_USE_XRANDR=0 -DJUCE_OPENGL3=1 -DJUCE_DSP_USE_SHARED_FFTW=0 -DJUCE_APP_VERSION=1.0.6 -DJUCE_APP_VERSION_HEX=0x10006 -DJUCE_USE_FREETYPE=1 -DJucePlugin_Build_VST=0 -DJucePlugin_Build_VST3=0 -DJucePlugin_Build_AU=0 -DJucePlugin_Build_AUv3=0 -DJucePlugin_Build_RTAS=0 -DJucePlugin_Build_AAX=0 -DJucePlugin_Build_Standalone=0 -DJucePlugin_Build_Unity=0"
inc="-I$ui -I$ui/juce/config -I$ui/juce/modules -I$ui/third_party -I$ui/third_party/concurrentqueue -I$vital/third_party -I$vital/third_party/json -I$vital/third_party/kissfft -I$ft/include"
for d in common common/wavetable interface/editor_components interface/editor_sections interface/look_and_feel interface/wavetable interface/wavetable/editors interface/wavetable/overlays; do inc="$inc -I$ui/src/$d"; done
for d in synth_engine effects filters framework lookups modulators modules producers utilities; do inc="$inc -I$vital/src/synthesis/$d"; done
units="$(for m in core events data_structures graphics gui_basics gui_extra opengl audio_basics audio_formats audio_processors audio_devices audio_utils dsp; do echo $ui/juce/config/include_juce_$m.cpp; done)
$ui/unity/common.cpp $ui/unity/interface_editor_components.cpp $ui/unity/interface_editor_sections.cpp $ui/unity/interface_editor_sections2.cpp $ui/unity/interface_look_and_feel.cpp $ui/unity/interface_wavetable.cpp
$vital/src/unity_build/synthesis.cpp $ui/juce/config/BinaryData.cpp $ui/rind_gui.cpp $vital/bridge.cpp"
[ $# -gt 1 ] && { shift; units="$*"; }
out=$(mktemp -d)
compile() {
  u=$1; log=$out/$(basename "$u").log
  if "$bin/$triple-clang++" -std=c++14 -fPIC -O1 -w -fexceptions -frtti -ffast-math -fno-strict-aliasing $extra $defs $inc $mode "$u" -o "$out/$(basename "$u").o" >"$log" 2>&1; then echo "ok    $(basename "$u")"; else echo "FAIL  $(basename "$u")"; fi
}
export -f compile; export out bin triple extra defs inc mode
echo "$units" | tr ' ' '\n' | grep -v '^$' | xargs -P "$(nproc)" -I{} bash -c 'compile {}' | sort
echo "---- errors ----"
grep -h "error" "$out"/*.log | sort | uniq -c | sort -rn | head -80
fails=$(grep -l "error" "$out"/*.log 2>/dev/null | wc -l)
# FreeType (C): the files crates/audio/build.rs builds
fts=0
for f in base/ftbase base/ftbbox base/ftbitmap base/ftdebug base/ftfstype base/ftgasp base/ftglyph base/ftinit base/ftmm base/ftstroke base/ftsynth base/ftsystem truetype/truetype sfnt/sfnt smooth/smooth cff/cff psaux/psaux pshinter/pshinter psnames/psmodule raster/raster; do
  "$bin/$triple-clang" -fPIC -O1 -w $extra -DFT2_BUILD_LIBRARY '-DFT_CONFIG_MODULES_H=<ftmodule.h>' -I$ft -I$ft/include $mode "$ft/src/$f.c" -o "$out/ft_$(echo $f | tr / _).o" 2>>"$out/ft.err" || fts=1
done
# With FULL=1 the objects are linked into a shared library that may leave nothing undefined: what the app would otherwise
# fail to load for on a phone.
if [ "${FULL:-0}" = 1 ] && [ $fails = 0 ] && [ $fts = 0 ]; then
  echo "---- link ----"
  "$bin/$triple-clang++" -shared -o "$out/libvital_ui.so" "$out"/*.o -Wl,--no-undefined -lEGL -lGLESv3 -llog -ldl -lm -latomic 2>"$out/link.err" || { echo "LINK FAILED"; sed 's/^.*undefined symbol/undefined symbol/' "$out/link.err" | sort | uniq -c | sort -rn | head -40; fails=1; }
fi
[ $fts = 1 ] && { echo "FreeType errors:"; head -20 "$out/ft.err"; }
exit $(( fails > 0 || fts > 0 ))
