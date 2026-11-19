// Stands in for Vital's load_save.h in this build. The real one is the preset, folder and settings code of the plugin
// (JUCE files, directories, GUI skins), which a host that has its own project format does not use. The wavetable code
// calls three of its static functions to update old wavetable files; those three are here (their bodies are Vital's own,
// in load_save_subset.cpp).
#pragma once

#include "JuceHeader.h"

#include <json/json.h>

using json = nlohmann::json;

class LoadSave {
  public:
    static void convertBufferToPcm(json& data, const std::string& field);
    static void convertPcmToFloatBuffer(json& data, const std::string& field);
    static int compareVersionStrings(String a, String b);
};
