/*
** Odin 2 Synthesizer Plugin
** Copyright (C) 2020 - 2021 TheWaveWarden
**
** Odin 2 is free software: you can redistribute it and/or modify
** it under the terms of the GNU General Public License as published by
** the Free Software Foundation, either version 3 of the License, or
** (at your option) any later version.
**
** Odin 2 is distributed in the hope that it will be useful,
** but WITHOUT ANY WARRANTY; without even the implied warranty of
** MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
** GNU General Public License for more details.
*/

#pragma once
#include "../JuceLibraryCode/JuceHeader.h"

// these 3 belong to the profiling macro
#include "ctime"
#include <iomanip> // setprecision
#include <sstream> // stringstream


#ifdef ODIN_WIN
#define M_PI 3.14159265359
#pragma warning(disable : 4244) //disable conversion warnings
#pragma warning(disable : 4100) //disable unreferenced parameter warning (VS really misses the point here...)
#pragma warning(disable : 4305) //disable double to float truncation warning
#pragma warning(disable : 4267) //disable size_t to int truncation warning
#endif

// shows the spectrum and wave display for wavetable osc 1
//#define WTGEN

#define ODIN_MAJOR_VERSION 2
#define ODIN_MINOR_VERSION 3
#define ODIN_PATCH_VERSION 4

#define ODIN_VERSION_STRING ("Odin 2." + std::to_string(ODIN_MINOR_VERSION) + "." + std::to_string(ODIN_PATCH_VERSION))

// ! this is for migrating patches from older version, not to be confused with above
#define ODIN_PATCH_MIGRATION_VERSION 5
//1 -> 2: replaced "amp width" by "amp velocity"
//2 -> 3: made carrier and modulator ratio, as well as pitchbend amount nonaudio values (but wrote migration code)

#define DBG_VAR(var) DBG(#var << ": " << var)

#define VOICES 24

#define MATRIX_SECTION_INDEX_PRESETS 10
#define MATRIX_SECTION_INDEX_ARP 1
#define MATRIX_SECTION_INDEX_MATRIX 0

// leave spare values for future additions :hype:
#define OSC_TYPE_ANALOG 2
#define OSC_TYPE_WAVETABLE 3
#define OSC_TYPE_MULTI 4
#define OSC_TYPE_VECTOR 5
#define OSC_TYPE_CHIPTUNE 6
#define OSC_TYPE_FM 7
#define OSC_TYPE_PM 8
#define OSC_TYPE_NOISE 15
#define OSC_TYPE_WAVEDRAW 20
#define OSC_TYPE_CHIPDRAW 21
#define OSC_TYPE_SPECDRAW 22

#define FILTER_TYPE_NONE 1
#define FILTER_TYPE_LP24 2
#define FILTER_TYPE_LP12 3
#define FILTER_TYPE_BP24 4
#define FILTER_TYPE_BP12 5
#define FILTER_TYPE_HP24 6
#define FILTER_TYPE_HP12 7
#define FILTER_TYPE_SEM12 15
#define FILTER_TYPE_DIODE 19
#define FILTER_TYPE_KORG_LP 24
#define FILTER_TYPE_KORG_HP 25
#define FILTER_TYPE_COMB 30
#define FILTER_TYPE_FORMANT 35
#define FILTER_TYPE_RINGMOD 40

// midpoint for filters:
// https://www.wolframalpha.com/input/?i=80*e%5E%28ln%2818000%2F80%29*0.5%29
// skew for filters:
// https://www.wolframalpha.com/input/?i=ln%280.5%29%2F%28ln%28%281200-80%29%2F%2818000-80%29%29%29
#define FILTER_SKEW_DEFAULT 0.25f

#define GAIN_SMOOTHIN_FACTOR 0.995f
#define PAN_SMOOTHIN_FACTOR 0.998f
#define PITCHBEND_SMOOTHIN_FACTOR 0.998f
#define FILTER_FREQ_SMOOTHING_FACTOR 0.998f
#define PAD_SMOOTHIN_FACTOR 0.998f
#define PWM_SMOOTHIN_FACTOR 0.998f
#define THRESHOLD_SMOOTHIN_FACTOR 0.998f

#define MINUS_12_dB_GAIN 0.251189f //needed for volume modulation threshold
#define PLUS_12_dB_GAIN 3.981072f

#define WAVEDRAW_STEPS_X 193
//#define SPECDRAW_STEPS_X 193 //(DISPLAY_WIDTH_SPEC - DRAW_INLAY_LEFT_SPEC - DRAW_INLAY_RIGHT_SPEC)
#define SPECDRAW_STEPS_X 49 //just enough so it fits the display
#define CHIPDRAW_STEPS_X 32

#define MODMATRIX_ROWS 9

#define VOL_MOD_UPPER_LIMIT 100

#define NUM_SP_AD1 15
#define NUM_SP_AD2 100

#define GETAUDIO(name) m_value_tree.getParameterAsValue(name).getValue()

#define SETAUDIO0TO1(name, value) m_value_tree.getParameter(name)->setValueNotifyingHost((float)value)

#define SETAUDIOFULLRANGE(name, value) /*m_value_tree.getParameter(name)->beginChangeGesture();*/                                                                                  \
	m_value_tree.getParameter(name)->setValueNotifyingHost(m_value_tree.getParameter(name)->convertTo0to1((float)value));                                                          \
	//m_value_tree.getParameter(name)->endChangeGesture();

#define SETAUDIOFULLRANGESAFE(name, value)                                                                                                                                         \
	if (m_value_tree.getParameter(name)) {                                                                                                                                         \
		SETAUDIOFULLRANGE(name, value)                                                                                                                                             \
	} else {                                                                                                                                                                       \
		DBG("Tried to access unknown audio-param:");                                                                                                                               \
		DBG(name);                                                                                                                                                                 \
	}

#define RETRIGGERAUDIO(name) SETAUDIOFULLRANGE(name, GETAUDIO(name))
#define SETAUDIOVALUEPATCH(name)

#define CLAMP(low, clamp, high) clamp = clamp < low ? low : clamp > high ? high : clamp

#define DBGAUDIO(string)                                                                                                                                                           \
	if (sample == 0) {                                                                                                                                                             \
		DBG(string);                                                                                                                                                               \
	}

// note: these are not the values stored in the value tree, since historically the param was a button (mono 0, poly 1) and is now a dropdown (can't be 0)
enum class PlayModes { Legato = 1, Poly = 2, Retrig = 3 };
#define PLAYMODETOVALUETREE(playmode) ((int)playmode - 1)
#define VALUETREETOPLAYMODE(mode) ((PlayModes)(mode + 1))

enum class GuiScale { Z100 = 4, Z125 = 5, Z150 = 6, Z175 = 7, Z200 = 8 };


