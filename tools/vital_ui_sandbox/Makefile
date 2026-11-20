# Builds Vital's original interface (vendor/vital-ui) on a desktop Linux box, windowless (EGL + OpenGL ES 3, Mesa),
# and draws it into a PNG: `make -j2 OBJ=/tmp/vui GLINC=/path/to/khronos/include && /tmp/vui/rind_ui out_prefix`.
# The Android build (crates/audio/build.rs) compiles the same files; this is how the interface is tried without a phone.
R := ../..
UI := $(R)/vendor/vital-ui
V := $(R)/vendor/vital
J := $(UI)/juce/modules
OBJ ?= obj
GLINC ?= /usr/include
DEFS := -DRIND_VITAL_UI=1 -DNO_AUTH=1 -DJUCE_HEADLESS_PLATFORM=1 -DOPENGL_ES=1 -DLINUX=1 -DNDEBUG=1 -DHEADLESS=0 -DJUCE_USE_XRANDR=0 -DJUCE_OPENGL3=1 -DJUCE_DSP_USE_SHARED_FFTW=0 -DJUCE_APP_VERSION=1.0.6 -DJUCE_APP_VERSION_HEX=0x10006 -DJucePlugin_Build_VST=0 -DJucePlugin_Build_VST3=0 -DJucePlugin_Build_AU=0 -DJucePlugin_Build_AUv3=0 -DJucePlugin_Build_RTAS=0 -DJucePlugin_Build_AAX=0 -DJucePlugin_Build_Standalone=0 -DJucePlugin_Build_Unity=0 -DFT2_BUILD_LIBRARY -DJUCE_USE_FREETYPE=1
VINC := $(foreach d,common common/wavetable interface/editor_components interface/editor_sections interface/look_and_feel interface/wavetable interface/wavetable/editors interface/wavetable/overlays,-I$(UI)/src/$(d))
SINC := $(foreach d,synth_engine effects filters framework lookups modulators modules producers utilities,-I$(V)/src/synthesis/$(d))
INC := -I$(UI) -I$(GLINC) -I$(UI)/juce/config -I$(J) $(VINC) $(SINC) -I$(V)/third_party -I$(V)/third_party/json -I$(V)/third_party/kissfft -I$(UI)/third_party -I$(UI)/third_party/concurrentqueue $(shell pkg-config --cflags freetype2)
CXXFLAGS := $(DEFS) $(INC) -std=c++14 -O1 -msse2 -ffast-math -pthread -w -fPIC
JMODS := core events data_structures graphics gui_basics gui_extra opengl audio_basics audio_formats audio_processors audio_devices audio_utils dsp
UNITY := common interface_editor_components interface_editor_sections interface_editor_sections2 interface_look_and_feel interface_wavetable
OBJS := $(addprefix $(OBJ)/juce_,$(addsuffix .o,$(JMODS))) $(addprefix $(OBJ)/v_,$(addsuffix .o,$(UNITY))) $(OBJ)/v_synthesis.o $(OBJ)/binary.o $(OBJ)/rind_gui.o $(OBJ)/bridge.o
all: $(OBJ)/rind_ui
$(OBJ)/juce_%.o: $(UI)/juce/config/include_juce_%.cpp
	@mkdir -p $(OBJ); $(CXX) $(CXXFLAGS) -c $< -o $@
$(OBJ)/v_synthesis.o: $(V)/src/unity_build/synthesis.cpp
	@mkdir -p $(OBJ); $(CXX) $(CXXFLAGS) -c $< -o $@
$(OBJ)/v_%.o: $(UI)/unity/%.cpp $(wildcard $(UI)/src/*/*.cpp $(UI)/src/*/*.h $(UI)/src/*/*/*.cpp $(UI)/src/*/*/*.h)
	@mkdir -p $(OBJ); $(CXX) $(CXXFLAGS) -c $< -o $@
$(OBJ)/binary.o: $(UI)/juce/config/BinaryData.cpp
	@mkdir -p $(OBJ); $(CXX) $(CXXFLAGS) -c $< -o $@
$(OBJ)/rind_gui.o: $(UI)/rind_gui.cpp $(UI)/rind_gui.h
	@mkdir -p $(OBJ); $(CXX) $(CXXFLAGS) -c $< -o $@
$(OBJ)/bridge.o: $(V)/bridge.cpp $(UI)/rind_gui.h
	@mkdir -p $(OBJ); $(CXX) $(CXXFLAGS) -c $< -o $@
$(OBJ)/main.o: main.cpp
	@mkdir -p $(OBJ); $(CXX) $(CXXFLAGS) -c $< -o $@
$(OBJ)/rind_ui: $(OBJS) $(OBJ)/main.o
	$(CXX) -o $@ $(OBJS) $(OBJ)/main.o -l:libEGL.so.1 -l:libGLESv2.so.2 $(shell pkg-config --libs freetype2) -ldl -lpthread -lrt
