[analogue]: <analogue.md>
[bulk-manifest]: <bulk-manifest.md>
[differential-event]: <differential-event.md>
[envbuf]: <envbuf.md>
[margin]: <margin.md>
[object-keys]: <../object-keys.md>

# BulkOrder

An *EnvBuf feed*[~][envbuf] in an *Analogue*[~][analogue]: complete, contiguous
*DifferentialEvents*[~][differential-event], followed by a *BulkManifest*[~][bulk-manifest]
footer and a *counter margin*[~][margin]. Its *object key*[~][object-keys] carries
the event interval and footer's HTTP byte range.
