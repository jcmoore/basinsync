[basin]: <basin.md>
[object-keys]: <../object-keys.md>
[reservoir]: <reservoir.md>
[revision]: <revision.md>
[viewpoint]: <viewpoint.md>

# Flow

A string-identified *Reservoir*[^][reservoir] within a *Basin*[^][basin]. Its
*Viewpoints*[^][viewpoint] begin at zero independently of other *Flows*.

Use a lowercase portable ASCII id, following the *Flow-id rules*[^][object-keys].
A UUIDv4 *revision*[^][revision] distinguishes its S3 namespace after that id.
