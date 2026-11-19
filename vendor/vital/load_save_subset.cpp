/* Copyright 2013-2019 Matt Tytel
 *
 * vital is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * vital is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with vital.  If not, see <http://www.gnu.org/licenses/>.
 */

// These three functions are copied from Vital's src/common/load_save.cpp without change (the rest of that file is the
// plugin's preset and folder handling).

#include "load_save.h"
#include "utils.h"

void LoadSave::convertBufferToPcm(json& data, const std::string& field) {
  if (data.count(field) == 0)
    return;

  MemoryOutputStream decoded;
  std::string wave_data = data[field];
  Base64::convertFromBase64(decoded, wave_data);
  int size = static_cast<int>(decoded.getDataSize()) / sizeof(float);
  std::unique_ptr<float[]> float_data = std::make_unique<float[]>(size);
  memcpy(float_data.get(), decoded.getData(), size * sizeof(float));
  std::unique_ptr<int16_t[]> pcm_data = std::make_unique<int16_t[]>(size);
  vital::utils::floatToPcmData(pcm_data.get(), float_data.get(), size);

  String encoded = Base64::toBase64(pcm_data.get(), sizeof(int16_t) * size);
  data[field] = encoded.toStdString();
}

void LoadSave::convertPcmToFloatBuffer(json& data, const std::string& field) {
  if (data.count(field) == 0)
    return;

  MemoryOutputStream decoded;
  std::string wave_data = data[field];
  Base64::convertFromBase64(decoded, wave_data);
  int size = static_cast<int>(decoded.getDataSize()) / sizeof(int16_t);
  std::unique_ptr<int16_t[]> pcm_data = std::make_unique<int16_t[]>(size);
  memcpy(pcm_data.get(), decoded.getData(), size * sizeof(int16_t));
  std::unique_ptr<float[]> float_data = std::make_unique<float[]>(size);
  vital::utils::pcmToFloatData(float_data.get(), pcm_data.get(), size);

  String encoded = Base64::toBase64(float_data.get(), sizeof(float) * size);
  data[field] = encoded.toStdString();
}

int LoadSave::compareVersionStrings(String a, String b) {
  a.trim();
  b.trim();

  if (a.isEmpty() && b.isEmpty())
    return 0;

  String major_version_a = a.upToFirstOccurrenceOf(".", false, true);
  String major_version_b = b.upToFirstOccurrenceOf(".", false, true);

  if (!major_version_a.containsOnly("0123456789"))
    major_version_a = "0";
  if (!major_version_b.containsOnly("0123456789"))
    major_version_b = "0";

  int major_value_a = major_version_a.getIntValue();
  int major_value_b = major_version_b.getIntValue();

  if (major_value_a > major_value_b)
    return 1;
  else if (major_value_a < major_value_b)
    return -1;
  return compareVersionStrings(a.fromFirstOccurrenceOf(".", false, true),
                               b.fromFirstOccurrenceOf(".", false, true));
}
