cmake_minimum_required(VERSION 3.15)
project(GoProWebcamFilter)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Force 64-bit compilation
set(CMAKE_VS_PLATFORM_NAME "x64")

# Windows SDK and DirectShow
set(WindowsSDK_DIR "C:/Program Files (x86)/Windows Kits/10")
include_directories(${WindowsSDK_DIR}/include/10.0.19041.0/ucrt)
include_directories(${WindowsSDK_DIR}/include/10.0.19041.0/um)
include_directories(${WindowsSDK_DIR}/include/10.0.19041.0/shared)

# Output directory
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY_DEBUG ${CMAKE_BINARY_DIR}/bin)
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE ${CMAKE_BINARY_DIR}/bin)

# Source files
set(SOURCES
    src/GoProCamFilter.cpp
    src/GoProCamPin.cpp
    src/GoProCamFilterImpl.cpp
)

# Header files
set(HEADERS
    src/GoProCamFilter.h
    src/GoProCamPin.h
    src/GoProCamFilterImpl.h
)

# Create the DLL
add_library(GoProCamFilter SHARED ${SOURCES} ${HEADERS})

# Link against Windows libraries
target_link_libraries(GoProCamFilter
    kernel32.lib
    user32.lib
    advapi32.lib
    ole32.lib
    oleaut32.lib
    strmiids.lib
    uuid.lib
)

# Preprocessor definitions for DirectShow
target_compile_definitions(GoProCamFilter PRIVATE
    _UNICODE
    UNICODE
    WIN32
    _WIN32_WINNT=0x0601
    _DEBUG
    DEBUG
    _WINDOWS
    _USRDLL
    GOProCamFilter_EXPORTS
)

# Disable certain warnings
target_compile_options(GoProCamFilter PRIVATE
    /W3
    /EHsc
    /MP
    /GS-
    /Gy
    /GR-
    /fp:fast
)

# Generate the.def file for exports
set_target_properties(GoProCamFilter PROPERTIES
    OUTPUT_NAME_DEBUG "GoProCamFilter"
    OUTPUT_NAME_RELEASE "GoProCamFilter"
    PREFIX ""
)

# Add post-build event to register the DLL
add_custom_command(TARGET GoProCamFilter POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E echo "Filter DLL built: $<TARGET_FILE:GoProCamFilter>"
)