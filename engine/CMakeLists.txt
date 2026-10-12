cmake_minimum_required(VERSION 3.25)
project(omnarc VERSION 0.1.0 LANGUAGES C CXX)

# llama.cpp with engine/patches. Backends are llama.cpp's own options, e.g. -DGGML_CUDA=ON or -DGGML_VULKAN=ON;
# Metal is on by default on Apple. It is added before the C++ standard is set: llama.cpp is C++17 and must stay so
# (MSVC would compile its u8 literals as C++20 char8_t).
include(cmake/llama.cmake)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_STANDARD 11)

if(MSVC)
  set(omnarc_warnings /W4 /WX)
  add_compile_definitions(_CRT_SECURE_NO_WARNINGS)  # getenv and friends are fine here
else()
  set(omnarc_warnings -Wall -Wextra -Wpedantic -Werror)
endif()

add_library(omnarc src/omnarc.cpp src/system.cpp)
target_include_directories(omnarc PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include)
target_link_libraries(omnarc PRIVATE llama ggml)
target_compile_options(omnarc PRIVATE ${omnarc_warnings})

# the accuracy gate: a model split into windows must match the same model run whole
add_executable(omnarc-gate tools/gate.cpp)
target_link_libraries(omnarc-gate PRIVATE omnarc ggml)
target_compile_options(omnarc-gate PRIVATE ${omnarc_warnings})

# the engine in its own process, driven by omnarc-node (engine/HOST.md)
add_executable(omnarc-engine tools/engine_main.cpp)
target_link_libraries(omnarc-engine PRIVATE omnarc ggml)
target_compile_options(omnarc-engine PRIVATE ${omnarc_warnings})
if(UNIX AND NOT APPLE AND NOT ANDROID)
  target_link_libraries(omnarc-engine PRIVATE rt)  # shm_open on older glibc
endif()

include(CTest)
if(BUILD_TESTING)
  add_executable(abi_test tests/abi_test.c)
  target_link_libraries(abi_test PRIVATE omnarc)
  set_target_properties(abi_test PROPERTIES LINKER_LANGUAGE CXX)
  add_test(NAME c_abi COMMAND abi_test)

  # tiny random-weight models of every splittable architecture; the windows must match the whole model exactly
  find_package(Python3 COMPONENTS Interpreter)
  if(Python3_FOUND)
    foreach(arch llama qwen3)
      set(model ${CMAKE_CURRENT_BINARY_DIR}/tiny-models/tiny-${arch}.gguf)
      add_test(NAME tiny_model_${arch}
        COMMAND ${Python3_EXECUTABLE} ${CMAKE_CURRENT_SOURCE_DIR}/tests/tiny_model.py
                --arch ${arch} --gguf-py ${OMNARC_LLAMA_SOURCE}/gguf-py --out ${model})
      set_tests_properties(tiny_model_${arch} PROPERTIES FIXTURES_SETUP tiny_${arch})

      # three windows of the 8 layers and the output head (layer 8), the hidden state passed exactly: on the CPU
      # the split run is bit for bit the whole run
      add_test(NAME window_${arch}
        COMMAND omnarc-gate --model ${model} --split 0:3,3:5,5:9 --random-tokens 192 --ctx 64 --wire f32
                --backend cpu --exact)
      # prima.cpp's ring: the head keeps the first layers and the output head, the hidden state travels as f16
      add_test(NAME window_${arch}_ring
        COMMAND omnarc-gate --model ${model} --split 0:2,2:6,6:8,8:9 --random-tokens 192 --ctx 64 --wire f16)
      set_tests_properties(window_${arch} window_${arch}_ring PROPERTIES FIXTURES_REQUIRED tiny_${arch})
    endforeach()
  endif()

  # a real model, e.g. stories15M-q8_0.gguf from huggingface.co/ggml-org (CI downloads it)
  set(OMNARC_TEST_MODEL "" CACHE FILEPATH "A small llama-architecture GGUF model for the accuracy gate on real weights")
  if(OMNARC_TEST_MODEL)
    set(story ${CMAKE_CURRENT_SOURCE_DIR}/tests/data/story.txt)
    add_test(NAME gate_real_exact
      COMMAND omnarc-gate --model ${OMNARC_TEST_MODEL} --split even:3 --text ${story} --ctx 128 --wire f32
              --backend cpu --exact)
    add_test(NAME gate_real_f16
      COMMAND omnarc-gate --model ${OMNARC_TEST_MODEL} --split even:2 --text ${story} --ctx 128 --wire f16)
    # every GPU backend in the build is asked for by name, so a missing device fails instead of falling back
    foreach(backend CUDA HIP VULKAN METAL)
      if(GGML_${backend})
        string(TOLOWER ${backend} name)
        add_test(NAME gate_real_${name}
          COMMAND omnarc-gate --model ${OMNARC_TEST_MODEL} --split even:3 --text ${story} --ctx 128 --wire f16
                  --backend ${name})
      endif()
    endforeach()
  endif()
endif()
