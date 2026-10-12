#include "system.h"

#include <algorithm>
#include <thread>

#if defined(_WIN32)
#  ifndef NOMINMAX
#    define NOMINMAX
#  endif
#  ifndef WIN32_LEAN_AND_MEAN
#    define WIN32_LEAN_AND_MEAN
#  endif
#  include <windows.h>
#else
#  include <fcntl.h>
#  include <unistd.h>
#endif

#if defined(__linux__)
#  include <fstream>
#  include <unordered_set>
#endif

#if defined(__APPLE__)
#  include <sys/sysctl.h>
#endif

namespace birlik {

uint32_t physical_cores() {
#if defined(__linux__)
    // one entry per set of thread siblings = one physical core
    std::unordered_set<std::string> siblings;
    for (uint32_t cpu = 0;; ++cpu) {
        std::ifstream file("/sys/devices/system/cpu/cpu" + std::to_string(cpu) + "/topology/thread_siblings");
        if (!file.is_open()) {
            break;
        }
        std::string line;
        if (std::getline(file, line)) {
            siblings.insert(line);
        }
    }
    if (!siblings.empty()) {
        return static_cast<uint32_t>(siblings.size());
    }
#elif defined(__APPLE__)
    int32_t cores = 0;
    size_t len = sizeof(cores);
    // performance cores first: efficiency cores slow down lockstep threads
    if (sysctlbyname("hw.perflevel0.physicalcpu", &cores, &len, nullptr, 0) == 0 && cores > 0) {
        return static_cast<uint32_t>(cores);
    }
    if (sysctlbyname("hw.physicalcpu", &cores, &len, nullptr, 0) == 0 && cores > 0) {
        return static_cast<uint32_t>(cores);
    }
#elif defined(_WIN32)
    DWORD size = 0;
    GetLogicalProcessorInformationEx(RelationProcessorCore, nullptr, &size);
    if (size > 0) {
        std::vector<char> buffer(size);
        auto *info = reinterpret_cast<PSYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX>(buffer.data());
        if (GetLogicalProcessorInformationEx(RelationProcessorCore, info, &size)) {
            uint32_t cores = 0;
            for (DWORD offset = 0; offset < size;) {
                auto *entry = reinterpret_cast<PSYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX>(buffer.data() + offset);
                if (entry->Relationship == RelationProcessorCore) {
                    cores++;
                }
                offset += entry->Size;
            }
            if (cores > 0) {
                return cores;
            }
        }
    }
#endif
    const unsigned int threads = std::thread::hardware_concurrency();
    return threads == 0 ? 4 : (threads <= 4 ? threads : threads / 2);
}

namespace {

std::vector<file_prefetcher::range> merge(std::vector<file_prefetcher::range> ranges) {
    std::sort(ranges.begin(), ranges.end());
    std::vector<file_prefetcher::range> merged;
    for (const auto &r : ranges) {
        if (r.second <= r.first) {
            continue;
        }
        if (!merged.empty() && r.first <= merged.back().second) {
            merged.back().second = std::max(merged.back().second, r.second);
        } else {
            merged.push_back(r);
        }
    }
    return merged;
}

#if defined(_WIN32)

// Windows has no read-ahead hint for a file handle, so the file is mapped and PrefetchVirtualMemory (Windows 8+)
// reads the ranges of the view; the pages are shared with llama.cpp's own mapping of the file.
class windows_prefetcher final : public file_prefetcher {
public:
    windows_prefetcher(HANDLE file, HANDLE mapping, void *view) : file_(file), mapping_(mapping), view_(view) {}

    ~windows_prefetcher() override {
        UnmapViewOfFile(view_);
        CloseHandle(mapping_);
        CloseHandle(file_);
    }

    void prefetch() override {
        using prefetch_fn = BOOL(WINAPI *)(HANDLE, ULONG_PTR, PWIN32_MEMORY_RANGE_ENTRY, ULONG);
        static const auto fn = reinterpret_cast<prefetch_fn>(
            reinterpret_cast<void *>(GetProcAddress(GetModuleHandleW(L"kernel32.dll"), "PrefetchVirtualMemory")));
        if (fn == nullptr || ranges_.empty()) {
            return;
        }
        std::vector<WIN32_MEMORY_RANGE_ENTRY> entries;
        entries.reserve(ranges_.size());
        for (const auto &r : ranges_) {
            entries.push_back({static_cast<char *>(view_) + r.first, static_cast<SIZE_T>(r.second - r.first)});
        }
        fn(GetCurrentProcess(), entries.size(), entries.data(), 0);
    }

private:
    HANDLE file_;
    HANDLE mapping_;
    void *view_;
};

#else

class posix_prefetcher final : public file_prefetcher {
public:
    explicit posix_prefetcher(int fd) : fd_(fd) {}

    ~posix_prefetcher() override { close(fd_); }

    void prefetch() override {
        for (const auto &r : ranges_) {
#if defined(__APPLE__)
            // F_RDADVISE takes an int count, so large ranges go in pieces
            constexpr uint64_t piece = 1u << 30;
            for (uint64_t off = r.first; off < r.second; off += piece) {
                radvisory advice{};
                advice.ra_offset = static_cast<off_t>(off);
                advice.ra_count = static_cast<int>(std::min(piece, r.second - off));
                fcntl(fd_, F_RDADVISE, &advice);
            }
#else
            posix_fadvise(fd_, static_cast<off_t>(r.first), static_cast<off_t>(r.second - r.first),
                          POSIX_FADV_WILLNEED);
#endif
        }
    }

private:
    int fd_;
};

#endif

}  // namespace

std::unique_ptr<file_prefetcher> file_prefetcher::open(const std::string &path, std::vector<range> ranges) {
    std::unique_ptr<file_prefetcher> result;
#if defined(_WIN32)
    const int wide_len = MultiByteToWideChar(CP_UTF8, 0, path.c_str(), -1, nullptr, 0);
    std::wstring wide(static_cast<size_t>(wide_len > 0 ? wide_len : 1), L'\0');
    MultiByteToWideChar(CP_UTF8, 0, path.c_str(), -1, wide.data(), wide_len);
    HANDLE file = CreateFileW(wide.c_str(), GENERIC_READ, FILE_SHARE_READ, nullptr, OPEN_EXISTING,
                              FILE_ATTRIBUTE_NORMAL, nullptr);
    if (file == INVALID_HANDLE_VALUE) {
        return nullptr;
    }
    HANDLE mapping = CreateFileMappingW(file, nullptr, PAGE_READONLY, 0, 0, nullptr);
    if (mapping == nullptr) {
        CloseHandle(file);
        return nullptr;
    }
    void *view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
    if (view == nullptr) {
        CloseHandle(mapping);
        CloseHandle(file);
        return nullptr;
    }
    result = std::make_unique<windows_prefetcher>(file, mapping, view);
#else
    const int fd = ::open(path.c_str(), O_RDONLY);
    if (fd < 0) {
        return nullptr;
    }
    result = std::make_unique<posix_prefetcher>(fd);
#endif
    result->ranges_ = merge(std::move(ranges));
    for (const auto &r : result->ranges_) {
        result->bytes_ += r.second - r.first;
    }
    return result;
}

}  // namespace birlik
