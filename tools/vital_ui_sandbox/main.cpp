// Sandbox proof: Vital's real interface, driven through the bridge's C face exactly as the app does (the engine rendered
// on an "audio thread", touches sent in, frames taken out), drawn windowless and written to PNGs.
#include <atomic>
#include <chrono>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <sstream>
#include <thread>
#include <vector>

extern "C" {
void* vital_new_keyed(double, unsigned long long);
void vital_free(void*);
void vital_note_on(void*, int, float, int);
void vital_render(void*, float*, float*, unsigned);
int vital_load_json(void*, const char*, size_t);
int vital_gui_open(unsigned long long, int, int);
void vital_gui_close(unsigned long long);
void vital_gui_pointer(unsigned long long, int, float, float, int, float, long long);
unsigned long long vital_gui_take_frame(unsigned long long, unsigned char*, unsigned long long, int*, int*);
int vital_gui_changes(unsigned long long, int*, float*, int);
int vital_gui_slots(unsigned long long, char*, int);
}

#define STB_IMAGE_WRITE_IMPLEMENTATION
static void write_ppm(const char* path, const unsigned char* rgba, int w, int h) {
  std::ofstream f(path, std::ios::binary);
  f << "P6\n" << w << " " << h << "\n255\n";
  for (int i = 0; i < w * h; ++i) f.write(reinterpret_cast<const char*>(rgba + 4 * i), 3);
}

int main(int argc, char** argv) {
  setenv("HOME", "/tmp/vh", 1);
  const unsigned long long key = 7;
  void* vital = vital_new_keyed(48000.0, key);
  std::atomic<bool> run{true};
  std::atomic<float> peak{0};
  std::thread audio([&] {
    std::vector<float> l(128), r(128);
    while (run) {
      vital_render(vital, l.data(), r.data(), 128);
      float p = 0;
      for (float v : l) p = std::max(p, std::abs(v));
      if (p > peak) peak = p;
      std::this_thread::sleep_for(std::chrono::microseconds(2600));
    }
  });
  if (!vital_gui_open(key, 1400, 820)) { puts("open failed"); return 1; }
  vital_note_on(vital, 60, 0.8f, 0);
  std::vector<unsigned char> buf(1400 * 820 * 4);
  int w = 0, h = 0, saved = 0;
  auto t0 = std::chrono::steady_clock::now();
  bool clicked = false;
  while (std::chrono::steady_clock::now() - t0 < std::chrono::seconds(12) && saved < 3) {
    auto id = vital_gui_take_frame(key, buf.data(), buf.size(), &w, &h);
    auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now() - t0).count();
    if (id && ms > 1500 && saved == 0) { write_ppm((std::string(argv[1]) + "a.ppm").c_str(), buf.data(), w, h); ++saved; printf("frame a id %llu %dx%d\n", id, w, h); }
    if (ms > 3000 && !clicked) {
      // drag the first knob-ish area: press, move, release over the OSC 1 level region
      long long t = ms;
      vital_gui_pointer(key, 1, 215, 215, 16, 1, t);
      for (int i = 1; i <= 10; ++i) vital_gui_pointer(key, 0, 215, 215 - 4 * i, 16, 1, t + 16 * i);
      vital_gui_pointer(key, 2, 215, 175, 0, 0, t + 200);
      clicked = true;
    }
    if (id && ms > 5000 && saved == 1) { write_ppm((std::string(argv[1]) + "b.ppm").c_str(), buf.data(), w, h); ++saved; printf("frame b id %llu\n", id); }
    if (id && ms > 8000 && saved == 2) { write_ppm((std::string(argv[1]) + "c.ppm").c_str(), buf.data(), w, h); ++saved; printf("frame c id %llu\n", id); }
    std::this_thread::sleep_for(std::chrono::milliseconds(5));
  }
  int idx[64]; float val[64];
  int n = vital_gui_changes(key, idx, val, 64);
  printf("gui changes: %d", n);
  for (int i = 0; i < n && i < 5; ++i) printf(" [%d=%g]", idx[i], val[i]);
  printf("\npeak audio %.3f\n", (double)peak);
  run = false; audio.join();
  vital_gui_close(key);
  vital_free(vital);
  puts("done");
  return 0;
}
