# llama.cpp with the Omnarc patch set.
#
# The submodule in third_party/llama.cpp stays unmodified. At configure time the parts needed for the library are
# copied into the build directory and patches/*.patch are applied there with `git apply`. A stamp holds the
# upstream commit and the patch contents, so the copy is redone only when one of them changes.
#
# For patch development, point OMNARC_LLAMA_DIR at a llama.cpp checkout that already has the patches applied as
# commits (see engine/patches/README.md); it is then built as is.

set(OMNARC_LLAMA_SOURCE "${CMAKE_CURRENT_SOURCE_DIR}/third_party/llama.cpp"
    CACHE PATH "Unmodified llama.cpp checkout (the submodule)")
set(OMNARC_LLAMA_DIR "" CACHE PATH "A llama.cpp tree that already has the Omnarc patches; empty = patch a copy")

file(GLOB OMNARC_PATCHES "${CMAKE_CURRENT_SOURCE_DIR}/patches/*.patch")
list(SORT OMNARC_PATCHES)
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS ${OMNARC_PATCHES})

if(OMNARC_LLAMA_DIR)
  set(omnarc_llama_dir "${OMNARC_LLAMA_DIR}")
else()
  if(NOT EXISTS "${OMNARC_LLAMA_SOURCE}/CMakeLists.txt")
    message(FATAL_ERROR "llama.cpp is missing in ${OMNARC_LLAMA_SOURCE}: run `git submodule update --init`")
  endif()

  find_package(Git REQUIRED)
  execute_process(COMMAND "${GIT_EXECUTABLE}" rev-parse HEAD
    WORKING_DIRECTORY "${OMNARC_LLAMA_SOURCE}"
    OUTPUT_VARIABLE omnarc_llama_commit OUTPUT_STRIP_TRAILING_WHITESPACE RESULT_VARIABLE omnarc_git_result)
  if(NOT omnarc_git_result EQUAL 0)
    set(omnarc_llama_commit "unknown")
  endif()

  set(omnarc_stamp_text "${omnarc_llama_commit}")
  foreach(patch IN LISTS OMNARC_PATCHES)
    file(SHA256 "${patch}" patch_hash)
    string(APPEND omnarc_stamp_text "\n${patch_hash}")
  endforeach()

  set(omnarc_llama_dir "${CMAKE_BINARY_DIR}/llama.cpp")
  set(omnarc_stamp "${omnarc_llama_dir}/.omnarc-patched")
  set(omnarc_old_stamp "")
  if(EXISTS "${omnarc_stamp}")
    file(READ "${omnarc_stamp}" omnarc_old_stamp)
  endif()

  if(NOT omnarc_old_stamp STREQUAL omnarc_stamp_text)
    message(STATUS "omnarc: patching llama.cpp ${omnarc_llama_commit}")
    file(REMOVE_RECURSE "${omnarc_llama_dir}")
    file(MAKE_DIRECTORY "${omnarc_llama_dir}")
    # only what the library build needs
    foreach(item CMakeLists.txt LICENSE cmake ggml include licenses src vendor)
      file(COPY "${OMNARC_LLAMA_SOURCE}/${item}" DESTINATION "${omnarc_llama_dir}")
    endforeach()
    # Inside a repository (the build directory usually is in the Omnarc checkout) `git apply` silently skips the
    # paths outside the current directory, so git must not see that repository; a reverse check then proves that
    # every patch is in place. Whitespace is ignored because Windows checkouts may have CRLF line endings.
    get_filename_component(omnarc_llama_parent "${omnarc_llama_dir}" DIRECTORY)
    set(omnarc_git ${CMAKE_COMMAND} -E env "GIT_CEILING_DIRECTORIES=${omnarc_llama_parent}" "${GIT_EXECUTABLE}")
    foreach(patch IN LISTS OMNARC_PATCHES)
      execute_process(COMMAND ${omnarc_git} apply --ignore-whitespace --whitespace=nowarn "${patch}"
        WORKING_DIRECTORY "${omnarc_llama_dir}" RESULT_VARIABLE apply_result ERROR_VARIABLE apply_error)
      if(apply_result EQUAL 0)
        execute_process(COMMAND ${omnarc_git} apply --ignore-whitespace --reverse --check "${patch}"
          WORKING_DIRECTORY "${omnarc_llama_dir}" RESULT_VARIABLE apply_result ERROR_VARIABLE apply_error)
      endif()
      if(NOT apply_result EQUAL 0)
        file(REMOVE_RECURSE "${omnarc_llama_dir}")
        message(FATAL_ERROR "omnarc: ${patch} does not apply to llama.cpp ${omnarc_llama_commit}:\n${apply_error}")
      endif()
    endforeach()
    file(WRITE "${omnarc_stamp}" "${omnarc_stamp_text}")
  endif()
endif()

# llama.cpp is built as a static library inside libomnarc; its tools, tests and servers are not needed
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
# report the commit of the Omnarc repository around the build directory instead
set(omnarc_git_ceiling "$ENV{GIT_CEILING_DIRECTORIES}")
if(NOT OMNARC_LLAMA_DIR)
  get_filename_component(omnarc_llama_parent "${omnarc_llama_dir}" DIRECTORY)
  set(ENV{GIT_CEILING_DIRECTORIES} "${omnarc_llama_parent}")
endif()
add_subdirectory("${omnarc_llama_dir}" "${CMAKE_BINARY_DIR}/llama.cpp-build" EXCLUDE_FROM_ALL SYSTEM)
set(ENV{GIT_CEILING_DIRECTORIES} "${omnarc_git_ceiling}")
