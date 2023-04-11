Command-line tool to analyze the number of significant bits in PCM-encoded audio files.

Arguments: <command> <file1> <file2>…
where <command> is:
  - `i`: show the most basic information and check it can be processed by symphonia
  - `inf`: show how many bits are used at all, how many zero bits have been stuffed behind each sample, for instance to fake 24 bit PCM from 16 bit PCM
  - `info`: like `inf` but also show how many distinct sample values occur (takes considerably longer)
  - `noise`: next to each input file, create WAV files called *<input file>+<#>bitsnoise.wav*. The first one *<input file>+0bitsnoise.wav* contains the PCM signal identical to the original but in the same format as the other files (without replaygain tags and such). Further ones have their least significant bits overwritten by pseudo-random noise. You can then compare them and assess how many bits you actually hear.

