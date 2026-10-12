# llama.cpp with the BIRLIK patch set.
#
# The submodule in third_party/llama.cpp stays unmodified. At configure time the parts needed for the library are
# copied into the build directory and patches/*.patch are applied there with `git apply`. A stamp holds the
# upstream commit and the patch contents, so the copy is redone only when one of them changes.
#
# For patch development, point BIRLIK_LLAMA_DIR at a llama.cpp checkout that already has the patches applied as
# commits (see engine/patches/README.md); it is then built as is.

set(BIRLIK_LLAMA_SOURCE "${CMAKE_CURRENT_SOURCE_DIR}/third_party/llama.cpp"
    CACHE PATH "Unmodified llama.cpp checkout (the submodule)")
set(BIRLIK_LLAMA_DIR "" CACHE PATH "A llama.cpp tree that already has the BIRLIK patches; empty = patch a copy")

file(GLOB BIRLIK_PATCHES "${CMAKE_CURRENT_SOURCE_DIR}/patches/*.patch")
list(SORT BIRLIK_PATCHES)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS ${BIRLIK_PATCHES})

if(BIRLIK_LLAMA_DIR)
  set(birlik_llama_dir "${BIRLIK_LLAMA_DIR}")
else()
  if(NOT EXISTS "${BIRLIK_LLAMA_SOURCE}/CMakeLists.txt")
    message(FATAL_ERROR "llama.cpp is missing in ${BIRLIK_LLAMA_SOURCE}: run `git submodule update --init`")
  endif()

  find_package(Git REQUIRED)
  execute_process(COMMAND "${GIT_EXECUTABLE}" rev-parse HEAD
    WORKING_DIRECTORY "${BIRLIK_LLAMA_SOURCE}"
    OUTPUT_VARIABLE birlik_llama_commit OUTPUT_STRIP_TRAILING_WHITESPACE RESULT_VARIABLE birlik_git_result)
  if(NOT birlik_git_result EQUAL 0)
    set(birlik_llama_commit "unknown")
  endif()

  set(birlik_stamp_text "${birlik_llama_commit}")
  foreach(patch IN LISTS BIRLIK_PATCHES)
    file(SHA256 "${patch}" patch_hash)
    string(APPEND birlik_stamp_text "\n${patch_hash}")
  endforeach()

  set(birlik_llama_dir "${CMAKE_BINARY_DIR}/llama.cpp")
  set(birlik_stamp "${birlik_llama_dir}/.birlik-patched")
  set(birlik_old_stamp "")
  if(EXISTS "${birlik_stamp}")
    file(READ "${birlik_stamp}" birlik_old_stamp)
  endif()

  if(NOT birlik_old_stamp STREQUAL birlik_stamp_text)
    message(STATUS "birlik: patching llama.cpp ${birlik_llama_commit}")
    file(REMOVE_RECURSE "${birlik_llama_dir}")
    file(MAKE_DIRECTORY "${birlik_llama_dir}")
    # only what the library build needs
    foreach(item CMakeLists.txt LICENSE cmake ggml include licenses src vendor)
      file(COPY "${BIRLIK_LLAMA_SOURCE}/${item}" DESTINATION "${birlik_llama_dir}")
    endforeach()
    # Inside a repository (the build directory usually is in the BIRLIK checkout) `git apply` silently skips the
    # paths outside the current directory, so git must not see that repository; a reverse check then proves that
    # every patch is in place. Whitespace is ignored because Windows checkouts may have CRLF line endings.
    get_filename_component(birlik_llama_parent "${birlik_llama_dir}" DIRECTORY)
    set(birlik_git ${CMAKE_COMMAND} -E env "GIT_CEILING_DIRECTORIES=${birlik_llama_parent}" "${GIT_EXECUTABLE}")
    foreach(patch IN LISTS BIRLIK_PATCHES)
      execute_process(COMMAND ${birlik_git} apply --ignore-whitespace --whitespace=nowarn "${patch}"
        WORKING_DIRECTORY "${birlik_llama_dir}" RESULT_VARIABLE apply_result ERROR_VARIABLE apply_error)
      if(apply_result EQUAL 0)
        execute_process(COMMAND ${birlik_git} apply --ignore-whitespace --reverse --check "${patch}"
          WORKING_DIRECTORY "${birlik_llama_dir}" RESULT_VARIABLE apply_result ERROR_VARIABLE apply_error)
      endif()
      if(NOT apply_result EQUAL 0)
        file(REMOVE_RECURSE "${birlik_llama_dir}")
        message(FATAL_ERROR "birlik: ${patch} does not apply to llama.cpp ${birlik_llama_commit}:\n${apply_error}")
      endif()
    endforeach()
    file(WRITE "${birlik_stamp}" "${birlik_stamp_text}")
  endif()
endif()

# llama.cpp is built as a static library inside libbirlik; its tools, tests and servers are not needed
set(BUILD_SHARED_LIBS OFF)
set(LLAMA_BUILD_COMMON OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_TESTS OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_TOOLS OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_SERVER OFF CACHE BOOL "" FORCE)
set(LLAMA_BUILD_APP OFF CACHE BOOL "" FORCE)
set(LLAMA_OPENSSL OFF CACHE BOOL "" FORCE)
set(GGML_CCACHE OFF CACHE BOOL "" FORCE)

# llama.cpp and ggml read their build commit with git; the patched copy is not a repository, and git must not
# report the commit of the BIRLIK repository around the build directory instead
set(birlik_git_ceiling "$ENV{GIT_CEILING_DIRECTORIES}")
if(NOT BIRLIK_LLAMA_DIR)
  get_filename_component(birlik_llama_parent "${birlik_llama_dir}" DIRECTORY)
  set(ENV{GIT_CEILING_DIRECTORIES} "${birlik_llama_parent}")
endif()
add_subdirectory("${birlik_llama_dir}" "${CMAKE_BINARY_DIR}/llama.cpp-build" EXCLUDE_FROM_ALL SYSTEM)
set(ENV{GIT_CEILING_DIRECTORIES} "${birlik_git_ceiling}")
