// Odin 2's processor as one translation unit (as Odin's own PluginProcessor.cpp does), without the editor and JUCE.
#include "PluginProcessor.h"

#include "PluginProcessorNonAudioParams.cpp"
#include "PluginProcessorMigratePatch.cpp"
#include "PluginProcessorReadPatch.cpp"
#include "PluginProcessorSetModPointers.cpp"
#include "PluginProcessorValueChange.cpp"
#include "PluginProcessorMidi.cpp"
#include "PluginProcessorProcess.cpp"
#include "PluginProcessorConstructor.cpp"
#include "PluginProcessorMisc.cpp"
