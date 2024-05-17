# Rust Player (Currently unnamed)

Terminal music player for FLAC and mp3 files, powered by Rust.

## Installation and Usage

```
git clone git@github.com:mackenziedg/rustplayer.git && cargo install --path rustplayer/
```

Then run the program like

```
rustplayer /path/to/music/files
```

## Keybindings

- '↑'/'↓': Navigate song list
- '←'/'→': Seek through file
- 'Shift + →': Skip song
- '-'/'=': Adjust volume down/up
- 'Enter': Play selected song
- 'p': Play/pause playing song
- 's': Rescan folder
- '/': Filter song list by title/artist/album. 'Enter' closes the search menu

## TODO

- Playlists
- Silent seeking
- Shuffle
- Help menu
- Searching
- Visualizer
- Thorough testing
