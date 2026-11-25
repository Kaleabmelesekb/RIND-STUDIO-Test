// Honeydew's glue for Airwindows Consolidated (MIT): one effect of the table.
#pragma once
#include <functional>
#include <memory>

#include "airwin_consolidated_base.h"

struct Entry {
    const char *name;
    int params;
    std::function<std::unique_ptr<AirwinConsolidatedBase>()> make;
};
