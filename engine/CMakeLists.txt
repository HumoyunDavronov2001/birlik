cmake_minimum_required(VERSION 3.21)
project(birlik VERSION 0.1.0 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_C_STANDARD 11)

add_library(birlik src/birlik.cpp)
target_include_directories(birlik PUBLIC ${CMAKE_CURRENT_SOURCE_DIR}/include)

if(MSVC)
  target_compile_options(birlik PRIVATE /W4 /WX)
else()
  target_compile_options(birlik PRIVATE -Wall -Wextra -Wpedantic -Werror)
endif()

include(CTest)
if(BUILD_TESTING)
  add_executable(abi_test tests/abi_test.c)
  target_link_libraries(abi_test PRIVATE birlik)
  set_target_properties(abi_test PROPERTIES LINKER_LANGUAGE CXX)
  add_test(NAME c_abi COMMAND abi_test)
endif()
