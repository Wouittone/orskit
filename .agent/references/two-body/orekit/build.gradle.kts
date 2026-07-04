plugins {
    application
}

repositories {
    mavenCentral()
}

dependencies {
    implementation("org.orekit:orekit:13.1.6")
}

application {
    mainClass = "org.orskit.reference.TwoBodyReference"
}
