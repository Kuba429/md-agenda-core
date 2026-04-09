# Task Test

* [[target]]
    * this should be added to backlinks section of target file
    * and it should not include the header because it has only the link to the file

* [[target]]
    * this should be added to backlinks section of target file
        * and it should NOT be appended to the bullet created by the bullet link above
        * it should be its own bullet
        * even if target link may be duplicated

    * and it should also not include the header because it has only the link to the file

* [[target]] something other than the link
    * this should be added to backlinks section of target file
        * and it should NOT be appended to the bullet created by the bullet links above
        * it should be its own bullet
        * even if target link may be duplicated

    * this one should include the link bullet because it has content besides the link


* [[target]] [[target2]]
    * this should be added to backlinks sections of both target files
        * and it should NOT be appended to the bullet created by the bullet links above
        * it should be its own bullet
        * even if target link may be duplicated

    * this one should include the link bullet because it has content besides the link
        * even though the additnal content is a link too - it links to a different file than the one it will be displayed in
            * because of that, it includes meaningful context and should be included

Some content here
