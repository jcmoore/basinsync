[datashed]: <datashed.md>
[object-keys]: <../object-keys.md>
[reservoir]: <reservoir.md>
[revision]: <revision.md>
[userspace]: <userspace.md>
[viewpoint]: <viewpoint.md>

# Flow

A client-driven, string-identified *Reservoir*[~][reservoir] in a *Datashed's*[~][datashed]
*UserSpace*[~][userspace]. Its
*Viewpoints*[~][viewpoint] begin at zero independently of other *Flows*.

Use a lowercase portable ASCII id, following the *Flow-id rules*[~][object-keys].
A UUIDv4 *revision*[~][revision] distinguishes its S3 namespace after that id.
