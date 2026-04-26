# Target 2

Second target note.
<!-- BACKLINKS:START -->
## Backlinks

- [[multi-link]]
    - [[target]] and [[target2]]

- [[multi-link]]

- [[multi-link]]
    - [[target]] and [[target2]]

- [[multi-link]]

- [[double-link]]
    * [[target]] [[target2]]
        * header should be omitted when it only contains link to a file to which it's added
        * but this one contains links to other files as well

        * basically header should be omitted only if it contains just the link to the current file
            * and markdown syntax like bullets

- [[backlink-tasks-test]]
    * [[target]] [[target2]]
        * and it should NOT be appended to the bullet created by the bullet links above
        * it should be its own bullet
        * even if target link may be duplicated

        * even though the additnal content is a link too - it links to a different file than the one it will be displayed in
            * because of that, it includes meaningful context and should be included

Some content here

<!-- BACKLINKS:END -->
