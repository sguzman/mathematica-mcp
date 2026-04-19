############################################################
#
#           WSTP.cmake
#
# Copyright 1986 through 2025 by Wolfram Research Inc.
# All rights reserved
#
############################################################

cmake_minimum_required(VERSION 3.11)

include(${CMAKE_CURRENT_LIST_DIR}/WSTP-targets.cmake)

# Process a .tm file into a .c or .cpp file using the `wsprep` command-line tool.
#
# More information on the purpose and format of .tm files and `wsprep` can be found here:
#
#   https://reference.wolfram.com/language/tutorial/SettingUpExternalFunctionsToBeCalledFromTheWolframLanguage.html
function(prep_sourcefile INPUT_FILE OUTPUT_FILE)
	if(NOT IS_ABSOLUTE "${OUTPUT_FILE}")
		set(OUTPUT_FILE ${CMAKE_CURRENT_BINARY_DIR}/${OUTPUT_FILE})
	endif()

	# Create OUTPUT_FILE by running `wsprep` on INPUT_FILE.
	add_custom_command(OUTPUT ${OUTPUT_FILE}
		COMMAND WSTP::wsprep ${INPUT_FILE} > ${OUTPUT_FILE}
		DEPENDS ${INPUT_FILE} WSTP::wsprep
	)
endfunction()

# Create library aliases, for backwards compatibility with previous versions of
# WSTP.cmake which did not create library targets within the `WSTP::` namespace.

if(${CMAKE_VERSION} VERSION_LESS "3.18.0") 
	set_target_properties(WSTP::STATIC_LIBRARY PROPERTIES IMPORTED_GLOBAL TRUE)
	set_target_properties(WSTP::DYNAMIC_LIBRARY PROPERTIES IMPORTED_GLOBAL TRUE)
endif()

add_library(wstp64i4s   ALIAS  WSTP::STATIC_LIBRARY)
add_library(wstp64i4  ALIAS  WSTP::DYNAMIC_LIBRARY)
