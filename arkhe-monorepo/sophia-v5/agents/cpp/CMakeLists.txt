cmake_minimum_required(VERSION 3.16)
project(sophia_v5_agents LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Opções de build
option(SOPHIA_USE_OPENMP "Habilitar OpenMP" ON)
option(SOPHIA_HAVE_OPENCV "Habilitar pipeline de visão com OpenCV" OFF)

# Núcleo dos patches #19-22 (header-only)
add_library(sophia_patches INTERFACE)
target_include_directories(sophia_patches INTERFACE ${CMAKE_CURRENT_SOURCE_DIR})
target_compile_options(sophia_patches INTERFACE -Wall -Wextra)

if(SOPHIA_USE_OPENMP)
  find_package(OpenMP)
  if(OpenMP_CXX_FOUND)
    target_link_libraries(sophia_patches INTERFACE OpenMP::OpenMP_CXX)
  endif()
endif()

if(SOPHIA_HAVE_OPENCV)
  find_package(OpenCV REQUIRED COMPONENTS core imgproc dnn)
  add_library(sophia_opencv INTERFACE)
  target_include_directories(sophia_opencv INTERFACE ${CMAKE_CURRENT_SOURCE_DIR})
  target_link_libraries(sophia_opencv INTERFACE ${OpenCV_LIBS})
endif()

add_executable(sophia_orchestrator main_sophia.cpp)
target_link_libraries(sophia_orchestrator PRIVATE sophia_patches)

add_executable(sophia_vision main_vision.cpp)
target_link_libraries(sophia_vision PRIVATE sophia_patches)
if(SOPHIA_HAVE_OPENCV)
  target_link_libraries(sophia_vision PRIVATE sophia_opencv)
else()
  # Sem OpenCV: compila pipeline em modo simulado (sem dep do OpenCV).
endif()