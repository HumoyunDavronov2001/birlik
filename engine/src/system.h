// System facts the engine needs: CPU cores and reading file ranges ahead of use.
#pragma once

#include <cstdint>
#include <memory>
#include <string>
#include <utility>
#include <vector>

namespace birlik {

// Physical CPU cores (hyperthreads are not useful for matrix math). Adapted from llama.cpp
// common/common.cpp, common_cpu_get_num_physical_cores (MIT).
uint32_t physical_cores();

// Asks the operating system to read byte ranges of a file into memory in the background (prima.cpp's prefetching,
// done on file ranges so that it does not depend on how llama.cpp maps the file). The pages land in the OS page
// cache, which every mapping of the file shares.
class file_prefetcher {
public:
    using range = std::pair<uint64_t, uint64_t>;  // [offset, offset + size)

    // Merges adjacent ranges; returns nullptr if the file cannot be opened.
    static std::unique_ptr<file_prefetcher> open(const std::string &path, std::vector<range> ranges);

    virtual ~file_prefetcher() = default;

    // Starts the reads and returns at once.
    virtual void prefetch() = 0;

    uint64_t bytes() const { return bytes_; }

protected:
    std::vector<range> ranges_;
    uint64_t bytes_ = 0;
};

}  // namespace birlik
