param([string] $Folder)
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$phrases = @(
    @('ru', 'next', 'следующий трек'),
    @('ru', 'volume', 'сделай громкость тридцать'),
    @('ru', 'album', 'включи альбом кино'),
    @('ru', 'pause', 'пауза'),
    @('ru', 'wave', 'включи мою волну'),
    @('ru', 'sleep', 'выключись через двадцать минут'),
    @('ru', 'wake', 'тактус'),
    @('ru', 'wake-next', 'тактус, следующий трек'),
    @('en', 'next', 'play the next song'),
    @('en', 'volume', 'set the volume to forty'),
    @('en', 'artist', 'play music by queen'),
    @('en', 'pause', 'pause'),
    @('en', 'wake', 'tactus'),
    @('en', 'wake-next', 'tact is, next song')
)
$voices = @{ ru = @('Dmitriy (Russian)', 'Vika (Russian)'); en = @('Microsoft David Desktop - English (United States)', 'Microsoft Zira Desktop - English (United States)') }
$speaker = New-Object -ComObject SAPI.SpVoice
$tokens = @{}
foreach ($token in $speaker.GetVoices()) { $tokens[$token.GetDescription()] = $token }
foreach ($phrase in $phrases) {
    $index = 0
    foreach ($name in $voices[$phrase[0]]) {
        $index++
        if (-not $tokens.ContainsKey($name)) { Write-Output "missing voice $name"; continue }
        $speaker.Voice = $tokens[$name]
        $format = New-Object -ComObject SAPI.SpAudioFormat
        $format.Type = 18
        $stream = New-Object -ComObject SAPI.SpFileStream
        $stream.Format = $format
        $path = Join-Path $Folder ($phrase[0] + '-' + $phrase[1] + '-' + $index + '.wav')
        $stream.Open($path, 3, $false)
        $speaker.AudioOutputStream = $stream
        [void]$speaker.Speak($phrase[2])
        $stream.Close()
    }
}
Get-ChildItem $Folder -Filter *.wav | ForEach-Object { $_.Name + ' ' + $_.Length }
